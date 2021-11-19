use rodio::{decoder::Decoder, OutputStream, Sink};
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;

fn main() -> Result<(), Box<dyn Error>> {
    let arg = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le chemin du fichier aac".into()),
    };

    let (_output_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let source = Decoder::new(BufReader::new(File::open(arg)?))?;
    sink.append(source);
    sink.sleep_until_end();

    Ok(())
}
