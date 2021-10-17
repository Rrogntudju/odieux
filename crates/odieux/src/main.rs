use hls_player::*;

static mut STATE: Option<State> = None;

struct
pub mod filters {
    use std::convert::Infallible;
    use std::path::PathBuf;
    use warp::Filter;

    pub fn static_file(path: PathBuf) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("static").and(warp::fs::dir(path))
    }
}

use std::env::{args, Args};
use std::net::SocketAddr;
use std::path::PathBuf;
use warp::Filter;
use anyhow::{Result, anyhow};
use filters::*;

fn parse_args(args: &mut Args) -> Result<(SocketAddr, PathBuf)> {
    let addr = match args.skip(1).next() {
        Some(arg) => arg.parse::<SocketAddr>()?,
        None => return Err(anyhow!("IP:Port est manquant")),
    };

    let path_static = match args.next() {
        Some(arg) => arg.parse::<PathBuf>()?,
        None => return Err(anyhow!("Le chemin du répertoire statique est manquant")),
    };

    if !path_static.is_dir() {
        return Err(anyhow!("{} n'existe pas ou n'est pas accessible", path_static.to_string_lossy()));
    }

    Ok((addr, path_static))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (addr, path_static) = parse_args(&mut args())?;
    let routes = static_file(path_static);
    let server = warp::serve(routes);
    server.run(addr).await;
    Ok(())
}