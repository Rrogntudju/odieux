use std::cell::RefCell;
use std::thread_local;
use hls_player::{start, Sink, OutputStream};
use gratte::{gratte, Episode};
use serde::{Deserialize, Serialize};


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
#[derive(Serialize)]
enum PlayerState {
    Playing,
    Paused,
    Stopped,
}
#[derive(Serialize)]
struct State {
    player: PlayerState,
    volume: usize,
    page: usize,
    episodes: Vec<Episode>,
}

thread_local! {
    static SINK: RefCell<Option<Sink>> = RefCell::new(None);
    static OUTPUTSTREAM: RefCell<Option<OutputStream>> = RefCell::new(None);
    static STATE: RefCell<Option<State>> = RefCell::new(None);
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
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};
    use bytes::Bytes;

    pub async fn command(body: Bytes,) -> Result<impl warp::Reply, Infallible> {
        let response = match serde_json::from_slice::<Command>(body.as_ref()) {
            Ok(command) => match command {
                Start(url) =>
                Volume(v) =>
                Pause =>
                Stop => 
                Play =>
                Page(p) =>
                State => reply_state();
            },
            _ => reply_error(StatusCode::BAD_REQUEST),
        };

        Ok(response)  
    }

    fn reply_error(sc: StatusCode) -> Result<Response<String>, Error> {
        Response::builder().status(sc).body(String::default())
    }

    fn reply_state() -> Result<Response<String>, Error> {
        Response::builder().status(200).body(String::default())
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
            .path("/statique/index.htm")
            .reply(&filters::static_file(PathBuf::from("../../statique")))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn command() {
        let resp = request()
            .method("POST")
            .path("/command")
            .body(r#"{"State"}"#)
            .reply(&filters::command())
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
