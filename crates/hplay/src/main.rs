use std::env::args;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir un url master.m3u8 validé".into()),
    };

    let (player, _output_stream) = hls_player::start(&arg)?;
    player.sleep_until_end();

    Ok(())
}
