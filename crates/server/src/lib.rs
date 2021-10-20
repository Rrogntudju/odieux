use std::cell::RefCell;
use std::collections::HashMap;
use std::thread_local;
use hls_player::{start, Sink, OutputStream};
use gratte::{gratte, Episode};

enum Command {
    Start(usize),
    Volume(usize),
    Pause,
    Stop,
    Play,
    Page(usize),
}

#[derive(Debug)]
enum PlayerState {
    Playing,
    Paused,
    Stopped,
}
struct State {
    player: String,
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
            .and(json_body())
            .and_then(handlers::command)
    }

    pub fn state() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("state")
            .and(warp::path::end())
            .and(warp::post())
            .and(json_body())
            .and_then(handlers::state)
    }

    fn json_body() -> impl Filter<Extract = (HashMap<String, String>,), Error = warp::Rejection> + Clone {
        warp::body::content_length_limit(1024).and(warp::body::json())
    }
}

mod handlers {
    use super::*;
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};

    pub async fn command(body: HashMap<String, String>,) -> Result<impl warp::Reply, Infallible> {
        
 
        Ok(Response::builder().status(200).body(String::default()))
    }

    pub async fn state(body: HashMap<String, String>,) -> Result<impl warp::Reply, Infallible> {
        Ok(Response::builder().status(200).body(String::default()))
    }


    fn reply_error(sc: StatusCode) -> Result<Response<String>, Error> {
        Response::builder().status(sc).body(String::default())
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
            .path("/userinfos")
            .body(r#"{"fournisseur": "Google", "origine": "http://localhost"}"#)
            .reply(&filters::command())
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn state() {
        let resp = request()
            .method("POST")
            .path("/userinfos")
            .body(r#"{"fournisseur": "Google", "origine": "http://localhost"}"#)
            .reply(&filters::state())
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
