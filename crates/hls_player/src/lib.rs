mod rxcursor;
use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use rxcursor::RxCursor;


pub fn start(url: &str) -> Result<Sink> {
    let rx = hls_handler::start(url)?;
    let (_output_stream, stream_handle) = OutputStream::try_default().context("Échec: création de OutputStream")?;
    let sink = Sink::try_new(&stream_handle).context("Échec: création de Sink")?;
    let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
    sink.append(source);

    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn ohdio() {
        let player = match start("Insérer un url master.m3u8 Ohdio validé") {
            Ok(player) => player,
            Err(e) => {
                println!("{:?}", e);
                return assert!(false);
            }
        };

        thread::sleep(Duration::from_secs(15));
        player.pause();
        thread::sleep(Duration::from_secs(3));
        player.play();
        thread::sleep(Duration::from_secs(3));
        player.set_volume(5.0);
        assert_eq!(player.volume(), 5.0);
        thread::sleep(Duration::from_secs(3));
        player.stop();
        // assert!(false); // pour visualiser le stdout
    }
}
