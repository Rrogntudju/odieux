use rodio::{OutputStreamBuilder, Sink, decoder::Decoder};
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le chemin du fichier aac".into()),
    };

    let output_stream = OutputStreamBuilder::open_default_stream()?;
    let sink = Sink::connect_new(output_stream.mixer());
    let source = Decoder::new(BufReader::new(File::open(arg)?))?;
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}
