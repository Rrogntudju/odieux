mod handler {
    use hls_player::{OutputStream, Sink};
    use media::{Episode, get_episodes, get_media_id};
    use serde::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::thread_local;

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
        page_no: usize,
        prog: usize,
        episodes: Vec<Episode>,
        message: String,
        en_lecture: Episode,
        en_lecture_prog: usize,
    }

    #[derive(Deserialize, PartialEq)]
    pub struct Pagination {
        page_no: usize,
        prog: usize,
        prog_id: usize,
    }

    #[derive(Deserialize, PartialEq)]
    pub enum Command {
        Start(Episode),
        Volume(usize),
        Pause,
        Stop,
        Play,
        Random(Pagination),
        Page(Pagination),
        State,
    }

    thread_local! {
        static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
        static OUTPUT_STREAM: RefCell<Option<OutputStream>> = const { RefCell::new(None) };
        static STATE: RefCell<State> = RefCell::new(State {
            player: PlayerState::Stopped,
            volume: 2,
            page_no: 0,
            prog: 0,
            episodes: Vec::new(),
            message: String::default(),
            en_lecture: Episode::default(),
            en_lecture_prog: 0,
        });
    }

    use anyhow::{Result, anyhow};
    use axum::{extract::Json, response::IntoResponse};
    use rand::Rng;
    use reqwest::Client;
    use serde_json::Value;
    use std::time::Duration;

    const TIME_OUT: u64 = 30;
    const URL_VALIDEUR_OD: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls&manifestVersion=2";
    const URL_VALIDEUR_LIVE: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls&manifestVersion=2";

    async fn start_player(episode_id: Option<&str>) -> Result<(Sink, OutputStream)> {
        let url = match episode_id {
            Some(episode_id) => {
                let media_id = get_media_id(episode_id).await?;
                URL_VALIDEUR_OD.replace("{}", &media_id)
            }
            None => URL_VALIDEUR_LIVE.to_owned(),
        };
        let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
        let response = client.get(&url).send().await?.text().await?;
        let value: Value = serde_json::from_str(&response)?;
        hls_player::start(value["url"].as_str().unwrap_or_default())
    }

    fn command_stop() {
        OUTPUT_STREAM.set(None);
        SINK.set(None);
        STATE.with_borrow_mut(|state| {
            state.player = PlayerState::Stopped;
            state.en_lecture = Episode::default();
        });
    }

    async fn command_start(episode: Episode) {
        command_stop();
        let result = if episode.titre == "En direct" {
            start_player(None).await
        } else if episode.id.is_empty() {
            Err(anyhow!("Aucune musique diffusée disponible"))
        } else {
            start_player(Some(&episode.id)).await
        };
        match result {
            Ok((new_sink, new_os)) => {
                SINK.set(Some(new_sink));
                OUTPUT_STREAM.set(Some(new_os));
                STATE.with_borrow_mut(|state| {
                    state.player = PlayerState::Playing;
                    state.en_lecture = episode;
                    state.en_lecture_prog = state.prog;
                    SINK.with_borrow(|sink| sink.as_ref().unwrap().set_volume(state.volume as f32 / 4.0));
                });
            }
            Err(e) => {
                let message = format!("{e:#}");
                eprintln!("{message}");
                STATE.with_borrow_mut(|state| state.message = message);
            }
        }
    }

    pub async fn execute(Json(command): Json<Command>) -> impl IntoResponse {
        if command != Command::State {
            STATE.with_borrow_mut(|state| state.message = String::default());
        }
        match command {
            Command::State => {
                // Vérifier si la lecture s'est terminée
                if STATE.with_borrow(|state| state.en_lecture != Episode::default()) && SINK.with_borrow(|sink| sink.as_ref().unwrap().empty()) {
                    if STATE.with_borrow(|state| state.en_lecture.titre == "En direct") {
                        command_start(Episode {
                            titre: "En direct".to_owned(),
                            id: "".to_owned(),
                        })
                        .await
                    } else {
                        command_stop()
                    }
                }
            }
            Command::Start(episode) => command_start(episode).await,
            Command::Page(pagination) => match get_episodes(pagination.prog_id, pagination.page_no).await {
                Ok(episodes) => STATE.with_borrow_mut(|state| {
                    state.episodes = episodes;
                    state.page_no = pagination.page_no;
                    state.prog = pagination.prog;
                }),
                Err(e) => STATE.with_borrow_mut(|state| state.message = format!("{e:#}")),
            },
            Command::Random(pagination) => {
                let page_no: usize = rand::rng().random_range(1..=pagination.page_no);
                match get_episodes(pagination.prog_id, page_no).await {
                    Ok(mut episodes) => {
                        let i = rand::rng().random_range(0..episodes.len());
                        command_start(episodes.swap_remove(i)).await;
                    }
                    Err(e) => STATE.with_borrow_mut(|state| state.message = format!("{e:#}")),
                }
            }
            Command::Volume(vol) => {
                if STATE.with_borrow(|state| state.player != PlayerState::Stopped) {
                    SINK.with_borrow(|sink| sink.as_ref().unwrap().set_volume(vol as f32 / 4.0));
                    STATE.with_borrow_mut(|state| state.volume = vol);
                }
            }
            Command::Play => {
                if STATE.with_borrow(|state| state.player == PlayerState::Paused) {
                    SINK.with_borrow(|sink| sink.as_ref().unwrap().play());
                    STATE.with_borrow_mut(|state| state.player = PlayerState::Playing);
                }
            }
            Command::Pause => {
                if STATE.with_borrow(|state| state.player == PlayerState::Playing) {
                    SINK.with_borrow(|sink| sink.as_ref().unwrap().pause());
                    STATE.with_borrow_mut(|state| state.player = PlayerState::Paused);
                }
            }
            Command::Stop => {
                if STATE.with_borrow(|state| state.player != PlayerState::Stopped) {
                    command_stop()
                }
            }
        }
        Json(STATE.with(|state| state.to_owned()))
    }
}

pub mod router {
    use super::handler::execute;
    use axum::{
        Router,
        routing::{get_service, post},
    };
    use std::path::PathBuf;
    use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir};

    // nest_service enlève le préfixe «statique» avant de passer la requête à serveDir
    pub fn app(path: PathBuf) -> Router {
        Router::new()
            .nest_service("/statique", get_service(ServeDir::new(path)))
            .route("/command", post(execute))
            .layer(RequestBodyLimitLayer::new(1024))
    }
}

#[cfg(test)]
mod tests {
    use super::router::app;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn statique() {
        let req = Request::builder().uri("/statique/odieux.htm").body(Body::empty()).unwrap();
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
