use anyhow::{bail, Result};
use server::routers::app;
use std::env::{args, Args};
use std::net::SocketAddr;
use std::path::PathBuf;

fn parse_args(args: &mut Args) -> Result<(SocketAddr, PathBuf)> {
    let erreur = "Args: <IP:Port> <chemin du répertoire statique>";
    let addr = match args.nth(1) {
        Some(arg) => arg.parse::<SocketAddr>()?,
        None => bail!(erreur),
    };

    let path_static = match args.next() {
        Some(arg) => arg.parse::<PathBuf>()?,
        None => bail!(erreur),
    };

    if !path_static.is_dir() {
        bail!("{} n'existe pas ou n'est pas accessible", path_static.to_string_lossy());
    }

    Ok((addr, path_static))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let (addr, path_static) = parse_args(&mut args())?;
    axum::Server::bind(&addr).serve(app(path_static).into_make_service()).await?;
    Ok(())
}
