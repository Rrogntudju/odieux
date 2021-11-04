use gratte::{gratte, Episode};
use hls_player::{OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::thread_local;

#[derive(Deserialize)]
enum Command {
    Start(Episode),
    Volume(usize),
    Pause,
    Stop,
    Play,
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
    use anyhow::{Context, Result};
    use bytes::Bytes;
    use serde_json::value::Value;
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};

    const TIME_OUT: u64 = 10;
    const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
    const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";

    fn start(id: &str) -> Result<(Sink, OutputStream)> {
        let url = URL_VALIDEUR.replace("{}", id);
        let value: Value = minreq::get(&url)
            .with_timeout(TIME_OUT)
            .send()
            .context(format!("Échec: get {}", url))?
            .json()?;
        hls_player::start(value["url"].as_str().unwrap_or_default()).context("Échec du démarrage")
    }

    pub async fn command(body: Bytes) -> Result<impl warp::Reply, Infallible> {
        let response = match serde_json::from_slice::<Command>(body.as_ref()) {
            Ok(command) => {
                STATE.with(|state| state.borrow_mut().message = String::default());
                match command {
                    Command::Start(épisode) => {
                        SINK.with(|sink| {
                            if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                                sink.borrow().as_ref().unwrap().stop();
                                // Set stopped state
                                STATE.with(|state| {
                                    let mut s = state.borrow_mut();
                                    s.player = PlayerState::Stopped;
                                    s.en_lecture = Episode::default();
                                });
                            }
                        });
                        match start(&épisode.media_id) {
                            Ok((new_sink, new_os)) => {
                                SINK.with(|sink| *sink.borrow_mut() = Some(new_sink));
                                OUTPUT_STREAM.with(|output_stream| *output_stream.borrow_mut() = Some(new_os));
                                // Set playing state
                                STATE.with(|state| {
                                    let mut s = state.borrow_mut();
                                    s.player = PlayerState::Playing;
                                    s.en_lecture = épisode;
                                });
                            }
                            Err(e) => {
                                eprintln!("{}", e);
                                STATE.with(|state| state.borrow_mut().message = "Échec du démarrage".to_string());
                            }
                        };
                    }
                    Command::Volume(vol) => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                            sink.borrow().as_ref().unwrap().set_volume((vol as f32) / 2.0);
                            // Set volume
                            STATE.with(|state| state.borrow_mut().volume = vol);
                        }
                    }),
                    Command::Pause => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Playing) {
                            sink.borrow().as_ref().unwrap().pause();
                            // Set paused state
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Paused);
                        }
                    }),
                    Command::Stop => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                            sink.borrow().as_ref().unwrap().stop();
                            // Set stopped state
                            STATE.with(|state| {
                                let mut s = state.borrow_mut();
                                s.player = PlayerState::Stopped;
                                s.en_lecture = Episode::default();
                            });
                        }
                    }),
                    Command::Play => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Paused) {
                            sink.borrow().as_ref().unwrap().play();
                            // Set playing state
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Playing);
                        }
                    }),
                    Command::Page(page) => {
                        let épisodes = gratte(CSB, page).context("Échec du grattage").unwrap_or_else(|e| {
                            eprintln!("{}", e);
                            Vec::new()
                        });
                        if épisodes.is_empty() {
                            let message = format!("Erreur de la page {}", page);
                            STATE.with(|state| state.borrow_mut().message = message);
                        } else {
                            // Set page
                            STATE.with(|state| {
                                let mut s = state.borrow_mut();
                                s.episodes = épisodes;
                                s.page = page;
                            });
                        }
                    }
                    Command::State => {
                        // Vérifier si la lecture s'est terminée
                        if STATE.with(|state| state.borrow().en_lecture != Episode::default()) {
                            SINK.with(|sink| {
                                if sink.borrow().as_ref().unwrap().empty() {
                                    // Set stopped state
                                    STATE.with(|state| {
                                        let mut s = state.borrow_mut();
                                        s.player = PlayerState::Stopped;
                                        s.en_lecture = Episode::default();
                                    });
                                }
                            });
                        }
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
        let state = STATE.with(|state| state.borrow().clone());
        Response::builder().status(StatusCode::OK).body(serde_json::to_string(&state).unwrap())
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
