use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};

lazy_static! {
    static ref SINK: Arc<RwLock<HashMap<SessionId, Session>>> = Arc::new(RwLock::new(HashMap::new()));
}

pub mod filters {
    use super::*;
    use std::convert::Infallible;
    use std::path::PathBuf;
    use warp::filters::{cookie, header};
    use warp::Filter;

    pub fn static_file(path: PathBuf) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("static").and(warp::fs::dir(path))
    }
}

use server::filters::*;
use std::env::{args, Args};
use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use warp::Filter;

fn parse_args(args: &mut Args) -> Result<(SocketAddr, PathBuf), Box<dyn Error>> {
    let addr = match args.skip(1).next() {
        Some(arg) => arg.parse::<SocketAddr>()?,
        None => return Err("IP:Port est manquant".into()),
    };

    let path_static = match args.next() {
        Some(arg) => arg.parse::<PathBuf>()?,
        None => return Err("Le chemin du répertoire static est manquant".into()),
    };

    if !path_static.is_dir() {
        return Err(format!("{} n'existe pas ou n'est pas accessible", path_static.to_string_lossy()).into());
    }

    Ok((addr, path_static))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (addr, path_static) = parse_args(&mut args())?;
    let routes = static_file(path_static);
    let server = warp::serve(routes);
    server.run(addr).await;
    Ok(())
}