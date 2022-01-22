use gratte::{gratte, Episode};
use hls_player::{OutputStream, Sink};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::thread_local;
use std::time::Duration;

const TIME_OUT: u64 = 30;

#[derive(Deserialize, PartialEq)]
enum Command {
    Start(Episode),
    Volume(usize),
    Pause,
    Stop,
    Play,
    Random(usize),
    Page(usize),
    State,
}
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
    use std::path::PathBuf;
    use warp::Filter;

    pub fn static_file(path: PathBuf) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("statique").and(warp::fs::dir(path))
    }

    pub fn command() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("command")
            .and(warp::path::end())
            .and(warp::post())
            .and(warp::body::content_length_limit(1024))
            .and(warp::body::bytes())
            .and_then(handlers::command)
    }
}

mod handlers {
    use super::*;
    use anyhow::{anyhow, Context, Result};
    use bytes::Bytes;
    use rand::Rng;
    use serde_json::Value;
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};

    const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
    const URL_VALIDEUR_OD: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";
    const URL_VALIDEUR_LIVE: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls";

    async fn start_player(id: Option<&str>) -> Result<(Sink, OutputStream)> {
        let url = match id {
            Some(id) => URL_VALIDEUR_OD.replace("{}", id),
            None => URL_VALIDEUR_LIVE.to_owned(),
        };
        let response = &CLIENT.get(&url).send().await?.text().await?;
        let value: Value = serde_json::from_str(&response)?;
        hls_player::start(value["url"].as_str().unwrap_or_default())
    }

    fn command_stop() {
        SINK.with(|sink| {
            if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                sink.borrow().as_ref().unwrap().stop();
                STATE.with(|state| {
                    let mut s = state.borrow_mut();
                    s.player = PlayerState::Stopped;
                    s.en_lecture = Episode::default();
                });
            }
        });
    }

    async fn command_start(épisode: Episode) {
        command_stop();
        let result = if épisode.titre == "En direct" {
            start_player(None).await
        } else if épisode.media_id.is_empty() {
            Err(anyhow!("Aucune musique diffusée disponible"))
        } else {
            start_player(Some(&épisode.media_id)).await
        };
        match result {
            Ok((new_sink, new_os)) => {
                SINK.with(|sink| *sink.borrow_mut() = Some(new_sink));
                OUTPUT_STREAM.with(|output_stream| *output_stream.borrow_mut() = Some(new_os));
                STATE.with(|state| {
                    let mut s = state.borrow_mut();
                    s.player = PlayerState::Playing;
                    s.en_lecture = épisode;
                    SINK.with(|sink| sink.borrow().as_ref().unwrap().set_volume((s.volume as f32) / 2.0));
                });
            }
            Err(e) => {
                let message = format!("{e:#}");
                eprintln!("{message}");
                STATE.with(|state| state.borrow_mut().message = message);
            }
        }
    }

    pub async fn command(body: Bytes) -> Result<impl warp::Reply, Infallible> {
        let response = match serde_json::from_slice::<Command>(body.as_ref()) {
            Ok(command) => {
                if command != Command::State {
                    STATE.with(|state| state.borrow_mut().message = String::default());
                }
                match command {
                    Command::Start(épisode) => command_start(épisode).await,
                    Command::Volume(vol) => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                            sink.borrow().as_ref().unwrap().set_volume((vol as f32) / 2.0);
                            STATE.with(|state| state.borrow_mut().volume = vol);
                        }
                    }),
                    Command::Pause => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Playing) {
                            sink.borrow().as_ref().unwrap().pause();
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Paused);
                        }
                    }),
                    Command::Play => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Paused) {
                            sink.borrow().as_ref().unwrap().play();
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Playing);
                        }
                    }),
                    Command::Random(pages) => {
                        let page: usize = rand::thread_rng().gen_range(1..=pages);
                        match gratte(CSB, page, &CLIENT).await.context("Échec du grattage") {
                            Ok(mut épisodes) => {
                                let i = rand::thread_rng().gen_range(0..épisodes.len());
                                command_start(épisodes.swap_remove(i)).await;
                            }
                            Err(e) => STATE.with(|state| state.borrow_mut().message = format!("{e:#}")),
                        }
                    }
                    Command::Stop => command_stop(),
                    Command::Page(page) => match gratte(CSB, page, &CLIENT).await.context("Échec du grattage") {
                        Ok(épisodes) => STATE.with(|state| {
                            let mut s = state.borrow_mut();
                            s.episodes = épisodes;
                            s.page = page;
                        }),
                        Err(e) => STATE.with(|state| state.borrow_mut().message = format!("{e:#}")),
                    },
                    Command::State => {
                        let mut live_restart = false;
                        // Vérifier si la lecture s'est terminée
                        if STATE.with(|state| state.borrow().en_lecture != Episode::default()) {
                            SINK.with(|sink| {
                                if sink.borrow().as_ref().unwrap().empty() {
                                    STATE.with(|state| {
                                        if state.borrow().en_lecture.titre == "En direct" {
                                            live_restart = true;
                                        } else {
                                            let mut s = state.borrow_mut();
                                            s.player = PlayerState::Stopped;
                                            s.en_lecture = Episode::default();
                                        }
                                    });
                                }
                            });
                        }
                        // Redémarrer la lecture en direct suite à une interruption intempestive
                        if live_restart {
                            command_start(Episode {
                                titre: "En direct".to_owned(),
                                media_id: "".to_owned(),
                            })
                            .await;
                        };
                    }
                };
                reply_state()
            }
            _ => reply_error(StatusCode::BAD_REQUEST),
        };
        Ok(response)
    }

    fn reply_error(sc: StatusCode) -> Result<Response<String>, Error> {
        Response::builder().status(sc).body(String::default())
    }

    fn reply_state() -> Result<Response<String>, Error> {
        let state = STATE.with(|state| serde_json::to_string(state).unwrap());
        Response::builder().status(StatusCode::OK).body(state)
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
