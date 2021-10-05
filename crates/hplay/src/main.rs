use hls_player::Player;
use std::env::args;
use std::error::Error;
use std::io::stdin;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir un url master.m3u8 validé".into()),
    };

    let mut player = Player::start(&arg)?;

    let mut input = String::new();
    stdin().read_line(&mut input).unwrap_or_default();
    player.stop();

    Ok(())
}
