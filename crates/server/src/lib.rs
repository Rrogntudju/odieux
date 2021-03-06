use hls_player::{OutputStream, Sink};
use media::{get_episodes, Episode};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::thread_local;
use std::time::Duration;

const TIME_OUT: u64 = 30;

#[derive(Serialize, Clone, PartialEq)]
enum PlayerState {
    Playing,
    Paused,
    Stopped,
}
#[derive(Serialize, Clone)]
struct State {
    player: PlayerState,
    volume: usize,
    page: usize,
    episodes: Vec<Episode>,
    message: String,
    en_lecture: Episode,
}

#[derive(Deserialize, PartialEq)]
pub(crate) enum Command {
    Start(Episode),
    Volume(usize),
    Pause,
    Stop,
    Play,
    Random(usize),
    Page(usize),
    State,
}

thread_local! {
    static SINK: RefCell<Option<Sink>> = RefCell::new(None);
    static OUTPUT_STREAM: RefCell<Option<OutputStream>> = RefCell::new(None);
    static STATE: RefCell<State> = RefCell::new(State {
        player: PlayerState::Stopped,
        volume: 2,
        page: 0,
        episodes: Vec::new(),
        message: String::default(),
        en_lecture: Episode::default(),
    });
}

static CLIENT: Lazy<Client> = Lazy::new(|| Client::builder().timeout(Duration::from_secs(TIME_OUT)).build().unwrap());

pub mod filters {
    use super::*;
    use bytes::Bytes;
    use std::path::PathBuf;
    use warp::Filter;

    pub fn static_file(path: PathBuf) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("statique").and(warp::fs::dir(path))
    }

    fn command_body() -> impl Filter<Extract = (Command,), Error = warp::Rejection> + Clone {
        warp::body::content_length_limit(1024)
            .and(warp::body::bytes())
            .and_then(|body: Bytes| async move { serde_json::from_slice::<Command>(body.as_ref()).map_err(|_| warp::reject()) })
    }

    pub fn command() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("command").and(warp::post()).and(command_body()).and_then(handlers::execute)
    }
}

mod handlers {
    use super::*;
    use anyhow::{anyhow, Result};
    use rand::Rng;
    use serde_json::Value;
    use std::convert::Infallible;
    use warp::http::{Response, StatusCode};

    const CSB: &str = "https://services.radio-canada.ca/neuro/sphere/v1/audio/apps/products/programmes-v2/cestsibon/{}?context=web&pageNumber={}";
    const URL_VALIDEUR_OD: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";
    const URL_VALIDEUR_LIVE: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls";

    async fn start_player(id: Option<&str>) -> Result<(Sink, OutputStream)> {
        let url = match id {
            Some(id) => URL_VALIDEUR_OD.replace("{}", id),
            None => URL_VALIDEUR_LIVE.to_owned(),
        };
        let response = CLIENT.get(&url).send().await?.text().await?;
        let value: Value = serde_json::from_str(&response)?;
        hls_player::start(value["url"].as_str().unwrap_or_default())
    }

    fn command_stop() {
        OUTPUT_STREAM.with(|output_stream| *output_stream.borrow_mut() = None);
        SINK.with(|sink| *sink.borrow_mut() = None);
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.player = PlayerState::Stopped;
            state.en_lecture = Episode::default();
        });
    }

    async fn command_start(??pisode: Episode) {
        command_stop();
        let result = if ??pisode.titre == "En direct" {
            start_player(None).await
        } else if ??pisode.media_id.is_empty() {
            Err(anyhow!("Aucune musique diffus??e disponible"))
        } else {
            start_player(Some(&??pisode.media_id)).await
        };
        match result {
            Ok((new_sink, new_os)) => {
                SINK.with(|sink| *sink.borrow_mut() = Some(new_sink));
                OUTPUT_STREAM.with(|output_stream| *output_stream.borrow_mut() = Some(new_os));
                STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.player = PlayerState::Playing;
                    state.en_lecture = ??pisode;
                    SINK.with(|sink| sink.borrow().as_ref().unwrap().set_volume((state.volume as f32) / 2.0));
                });
            }
            Err(e) => {
                let message = format!("{e:#}");
                eprintln!("{message}");
                STATE.with(|state| state.borrow_mut().message = message);
            }
        }
    }

    pub(crate) async fn execute(command: Command) -> Result<impl warp::Reply, Infallible> {
        if command != Command::State {
            STATE.with(|state| state.borrow_mut().message = String::default());
        }
        match command {
            Command::Start(??pisode) => command_start(??pisode).await,
            Command::Volume(vol) => {
                if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                    SINK.with(|sink| sink.borrow().as_ref().unwrap().set_volume((vol as f32) / 2.0));
                    STATE.with(|state| state.borrow_mut().volume = vol);
                }
            }
            Command::Pause => {
                if STATE.with(|state| state.borrow().player == PlayerState::Playing) {
                    SINK.with(|sink| sink.borrow().as_ref().unwrap().pause());
                    STATE.with(|state| state.borrow_mut().player = PlayerState::Paused);
                }
            }
            Command::Play => {
                if STATE.with(|state| state.borrow().player == PlayerState::Paused) {
                    SINK.with(|sink| sink.borrow().as_ref().unwrap().play());
                    STATE.with(|state| state.borrow_mut().player = PlayerState::Playing);
                }
            }
            Command::Random(pages) => {
                let page: usize = rand::thread_rng().gen_range(1..=pages);
                let url = CSB.replace("{}", &format!("{page}"));
                match get_episodes(&url, &CLIENT).await {
                    Ok(mut ??pisodes) => {
                        let i = rand::thread_rng().gen_range(0..??pisodes.len());
                        command_start(??pisodes.swap_remove(i)).await;
                    }
                    Err(e) => STATE.with(|state| state.borrow_mut().message = format!("{e:#}")),
                }
            }
            Command::Stop => {
                if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                    command_stop()
                }
            }
            Command::Page(page) => {
                let url = CSB.replace("{}", &format!("{page}"));
                match get_episodes(&url, &CLIENT).await {
                    Ok(??pisodes) => STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.episodes = ??pisodes;
                        state.page = page;
                    }),
                    Err(e) => STATE.with(|state| state.borrow_mut().message = format!("{e:#}")),
                }
            }
            Command::State => {
                // V??rifier si la lecture s'est termin??e
                if STATE.with(|state| state.borrow().en_lecture != Episode::default()) && SINK.with(|sink| sink.borrow().as_ref().unwrap().empty()) {
                    if STATE.with(|state| state.borrow().en_lecture.titre == "En direct") {
                        command_start(Episode {
                            titre: "En direct".to_owned(),
                            media_id: "".to_owned(),
                        })
                        .await
                    } else {
                        command_stop()
                    }
                }
            }
        }
        let state = STATE.with(|state| serde_json::to_string(state).unwrap());
        Ok(Response::builder().status(StatusCode::OK).body(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn static_file() {
        let resp = request()
            .method("GET")
            .path("/statique/csb.htm")
            .reply(&filters::static_file(PathBuf::from("../../statique")))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn command() {
        let resp = request()
            .method("POST")
            .path("/command")
            .body(r#"{"State": null}"#)
            .reply(&filters::command())
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
