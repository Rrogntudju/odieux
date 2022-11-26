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

pub mod routers {
    use super::*;
    use axum::{
        http::StatusCode,
        response::IntoResponse,
        routing::{get_service, post},
        Router,
    };
    use std::path::PathBuf;
    use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir};

    async fn handle_error(_err: std::io::Error) -> impl IntoResponse {
        (StatusCode::INTERNAL_SERVER_ERROR, "DOH!")
    }

    pub fn app(path: PathBuf) -> Router {
        Router::new()
            .route("/statique", get_service(ServeDir::new(path)).handle_error(handle_error))
            .route("/command", post(handlers::execute))
            .layer(RequestBodyLimitLayer::new(1024))
    }
}

mod handlers {
    use super::*;
    use anyhow::{anyhow, Result};
    use axum::{extract::Json, http::StatusCode, response::IntoResponse};
    use rand::Rng;
    use serde_json::Value;

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
                    let mut state = state.borrow_mut();
                    state.player = PlayerState::Playing;
                    state.en_lecture = épisode;
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

    pub(crate) async fn execute(Json(command): Json<Command>) -> impl IntoResponse {
        if command != Command::State {
            STATE.with(|state| state.borrow_mut().message = String::default());
        }
        match command {
            Command::Start(épisode) => command_start(épisode).await,
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
                    Ok(mut épisodes) => {
                        let i = rand::thread_rng().gen_range(0..épisodes.len());
                        command_start(épisodes.swap_remove(i)).await;
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
                    Ok(épisodes) => STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.episodes = épisodes;
                        state.page = page;
                    }),
                    Err(e) => STATE.with(|state| state.borrow_mut().message = format!("{e:#}")),
                }
            }
            Command::State => {
                // Vérifier si la lecture s'est terminée
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
        (StatusCode::OK, state)
    }
}

#[cfg(test)]
mod tests {
    use crate::routers::app;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn static_file() {
        let req = Request::builder().uri("/statique/csb.htm").body(Body::empty()).unwrap();
        let resp = app("../../statique".into()).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn command() {
        let req = Request::builder()
            .uri("/command")
            .header("Content-Type", "application/json")
            .method("POST")
            .body(Body::from(r#"{"State": null}"#))
            .unwrap();
        let resp = app("../../statique".into()).oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
