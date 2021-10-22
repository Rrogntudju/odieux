use gratte::{gratte, Episode};
use hls_player::{start, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::thread_local;

#[derive(Deserialize)]
enum Command {
    Start(String),
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
}

thread_local! {
    static SINK: RefCell<Option<Sink>> = RefCell::new(None);
    static OUTPUTSTREAM: RefCell<Option<OutputStream>> = RefCell::new(None);
    static STATE: RefCell<State> = RefCell::new(State {
        player: PlayerState::Stopped,
        volume: 10,
        page: 1,
        episodes: Vec::new(),
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
    use bytes::Bytes;
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};
    use anyhow::Result;

    const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
    const VALIDATION: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";

    fn valider(url:&str, id: &str) -> Result<String> {

    }

    pub async fn command(body: Bytes) -> Result<impl warp::Reply, Infallible> {
        let response = match serde_json::from_slice::<Command>(body.as_ref()) {
            Ok(command) => {
                match command {
                    Command::Start(id) => (),
                    Command::Volume(vol) => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                            sink.borrow().as_ref().unwrap().set_volume((vol / 2) as f32);
                            STATE.with(|state| state.borrow_mut().volume = vol);
                        }
                    }),
                    Command::Pause => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Playing) {
                            sink.borrow().as_ref().unwrap().pause();
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Paused);
                        }
                    }),
                    Command::Stop => SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player != PlayerState::Stopped) {
                            sink.borrow().as_ref().unwrap().stop();
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Stopped);
                        }
                    }),
                    Command::Play =>SINK.with(|sink| {
                        if STATE.with(|state| state.borrow().player == PlayerState::Paused) {
                            sink.borrow().as_ref().unwrap().pause();
                            STATE.with(|state| state.borrow_mut().player = PlayerState::Playing);
                        }
                    }),
                    Command::Page(page) => (),
                    Command::State => (),
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
        /*         STATE.with(|state| {
            if state.borrow().is_none() {
                let épisodes = gratte(CSB, 1).unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    Vec::new()
                });
                *state.borrow_mut() = State {
                    player: PlayerState::Stopped,
                    volume: 100,
                    page: 1,
                    episodes: épisodes,
                };
            }
        }); */
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
