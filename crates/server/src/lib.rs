use serde_json::{Map, Value};

pub mod filters {
    use super::*;
    use std::convert::Infallible;
    use std::path::PathBuf;
    use warp::filters::{cookie, header};
    use warp::Filter;

    pub fn static_file(path: PathBuf) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("static").and(warp::fs::dir(path))
    }

    pub fn command() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("command")
            .and(warp::path::end())
            .and(warp::post())
            .and(json_body())
            .and_then(handlers::command)
    }

    fn json_body() -> impl Filter<Extract = (HashMap<String, String>,), Error = warp::Rejection> + Clone {
        warp::body::content_length_limit(1024).and(warp::body::json())
    }
}

mod handlers {
    use super::*;
    use std::convert::Infallible;
    use warp::http::{Error, Response, StatusCode};

    pub async fn command(
        csrf_cookie: Option<String>,
        csrf_header: Option<String>,
        session_cookie: Option<String>,
        body: HashMap<String, String>,
        sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    ) -> Result<impl warp::Reply, Infallible> {
        // Validation Csrf si le cookie Csrf est présent
        if let Some(ctoken) = csrf_cookie {
            match csrf_header {
                Some(htoken) if htoken == ctoken => (),
                Some(htoken) if htoken != ctoken => {
                    eprintln!("{0} != {1}", htoken, ctoken);
                    return Ok(reply_error(StatusCode::FORBIDDEN));
                }
                _ => {
                    eprintln!("X-Csrf-Token est absent");
                    return Ok(reply_error(StatusCode::FORBIDDEN));
                }
            }
        };

        let fournisseur = body.get("fournisseur").unwrap_or(&LOL);
        let origine = body.get("origine").unwrap_or(&LOL);

        let response = match session_cookie {
            Some(stoken) => {
                let id: SessionId = stoken.into();
                let lock = sessions.read().expect("Failed due to poisoned lock");

                match lock.get(&id) {
                    Some(session) if session.is_expired() => {
                        drop(lock);
                        eprintln!("userinfos: Session expirée ou pas authentifiée");
                        sessions.write().expect("Failed due to poisoned lock").remove(&id);
                        reply_redirect_fournisseur(fournisseur, origine, sessions)
                    }
                    Some(session) => {
                        match session {
                            Session::Authenticated(client, f, token) if f == fournisseur => {
                                let http = reqwest::Client::new();
                                let userinfo = match client.request_userinfo(&http, token) {
                                    Ok(userinfo) => userinfo,
                                    Err(e) => {
                                        eprintln!("{0}", e.to_string());
                                        return Ok(reply_error(StatusCode::INTERNAL_SERVER_ERROR));
                                    }
                                };
                                drop(lock);

                                let value = serde_json::to_value(&userinfo).unwrap_or_default();
                                let map = value.as_object().unwrap_or(&LOL_MAP);
                                let infos = Value::Array(
                                    map.into_iter()
                                        .filter_map(|(k, v)| match v.is_null() {
                                            true => None,
                                            false => {
                                                let mut map = serde_json::Map::new();
                                                map.insert("propriété".into(), Value::String(k.to_owned()));
                                                map.insert("valeur".into(), v.to_owned());
                                                Some(Value::Object(map))
                                            }
                                        })
                                        .collect::<Vec<Value>>(),
                                );

                                Response::builder()
                                    .status(StatusCode::OK)
                                    .body(serde_json::to_string(&infos).unwrap_or_default())
                            }
                            _ => {
                                // Changement de fournisseur
                                drop(lock);
                                sessions.write().expect("Failed due to poisoned lock").remove(&id);
                                reply_redirect_fournisseur(fournisseur, origine, sessions)
                            }
                        }
                    }
                    None => {
                        drop(lock);
                        eprintln!("userinfos: Pas de session");
                        reply_redirect_fournisseur(fournisseur, origine, sessions)
                    }
                }
            }
            None => reply_redirect_fournisseur(fournisseur, origine, sessions),
        };

        Ok(response)
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
            .path("/static/userinfos.htm")
            .reply(&filters::static_file(PathBuf::from("../static")))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn csrf_mismatch1() {
        let resp = request()
            .method("POST")
            .path("/userinfos")
            .header("Cookie", "Csrf-Token=LOL")
            .body(r#"{"fournisseur": "Google", "origine": "http://localhost"}"#)
            .reply(&filters::userinfos())
            .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
