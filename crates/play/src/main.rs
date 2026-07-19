use rodio::{DeviceSinkBuilder, Decoder, Player};
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le chemin du fichier aac".into()),
    };

    let sink = DeviceSinkBuilder::open_default_sink()?;
    let player = Player::connect_new(&sink.mixer());
    let source = Decoder::new(BufReader::new(File::open(arg)?))?;
    player.append(source);
    player.sleep_until_end();

    Ok(())
}
