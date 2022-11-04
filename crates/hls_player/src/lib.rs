#[cfg(not(feature = "throttling"))]
mod rxcursor;
#[cfg(feature = "throttling")]
mod rxcursor2;
use anyhow::{bail, Context, Result};
use rodio::Decoder;
pub use rodio::{OutputStream, Sink};
use rxcursor::RxCursor;

pub fn start(url: &str) -> Result<(Sink, OutputStream)> {
    let rx = hls_handler::start(url)?;
    let Ok((_output_stream, stream_handle)) = OutputStream::try_default() else {
        bail!("La sortie audio est déjà utilisée");
    };
    let sink = Sink::try_new(&stream_handle).context("Échec: création de Sink")?;
    let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
    sink.append(source);

    Ok((sink, _output_stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn ohdio() {
        let (player, _output_stream) = match start("Fournir un url master.m3u8 validé") {
            Ok((s, o)) => (s, o),
            Err(e) => {
                println!("{e:?}");
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
