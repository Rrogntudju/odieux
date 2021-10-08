mod rxcursor;
use rxcursor::RxCursor;
use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};

pub struct Player {
    _output_stream: OutputStream,
    sink: Sink,
}

impl Player {
    pub fn start(url: &str) -> Result<Self> {
        let rx = hls_handler::start(url)?;
        let (_output_stream, stream_handle) = OutputStream::try_default().context("Échec: création de OutputStream")?;
        let sink = Sink::try_new(&stream_handle).context("Échec: création de Sink")?;
        let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
        sink.append(source);

        Ok(Self { _output_stream, sink })
    }

    pub fn play(&mut self) {
        self.sink.play();
    }

    pub fn stop(&mut self)  {
        self.sink.stop();
    }

    pub fn pause(&mut self)  {
        self.sink.pause();
    }

    pub fn volume(&mut self) -> f32 {
        self.sink.volume()
    }

    pub fn set_volume(&mut self, volume: f32)  {
        self.sink.set_volume(volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    #[test]
    fn ohdio() {
        let mut player = match Player::start("Insérer un url master.m3u8 Ohdio validé") {
            Ok(player) => player,
            Err(e) => {
                println!("{:?}", e);
                return assert!(false);
            }
        };

        thread::sleep(time::Duration::from_secs(15));
        player.pause();
        thread::sleep(time::Duration::from_secs(3));
        player.play();
        thread::sleep(time::Duration::from_secs(3));
        player.set_volume(5.0);
        assert_eq!(player.volume(), 5.0);
        thread::sleep(time::Duration::from_secs(3));
        player.stop();
        // assert!(false); // pour visualiser le stdout
    }
}
