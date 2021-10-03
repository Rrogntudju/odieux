use anyhow::{anyhow, Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::{thread, time};

pub struct Player {
    output_stream: OutputStream,
    sink: Arc<Mutex<Sink>>,
    stop_signal: Arc<AtomicBool>,
}

impl Player {
    pub fn start(url: &str) -> Result<Self> {
        let rx = hls_handler::start(url)?;
        let (output_stream, stream_handle) = OutputStream::try_default().context("Échec: création de OutputStream")?;
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle).context("Échec: création de Sink")?));
        let sink2 = sink.clone();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal2 = stop_signal.clone();

        thread::spawn(move || {
            while !stop_signal2.load(Ordering::Relaxed) {
                let sink = match sink2.lock() {
                    Ok(sink) => sink,
                    Err(e) => return eprintln!("Sink lock:\n{:?}", e)
                };

                if sink.len() < 2 {
                    match rx.recv() {
                        Ok(message) => {
                            let stream = match message.context("Échec: réception du message") {
                                Ok(stream) => stream,
                                Err(e) => return eprintln!("{:?}", e)
                            };
                            let source = match Decoder::new(Cursor::new(*stream)).context("Échec: création de Decoder") {
                                Ok(source) => source,
                                Err(e) => return eprintln!("{:?}", e)
                            };
                            sink.append(source);
                        }
                        Err(_) => return, // tx was dropped
                    }
                }
                drop(sink);

                thread::sleep(time::Duration::from_millis(1000));
            }
        });

        Ok(Self { output_stream, sink, stop_signal })
    }

    pub fn play(&mut self) -> Result<()> {
        match self.sink.lock() {
            Ok(sink) => sink.play(),
            Err(e) => return Err(anyhow!("Sink lock:\n{:?}", e)),
        }

        Ok(())
    }

    pub fn stop(&mut self)  {
        match self.sink.lock() {
            Ok(sink) => sink.pause(),
            Err(e) => return eprintln!("Sink lock:\n{:?}", e),
        }

        self.stop_signal.store(true, Ordering::Relaxed);
    }

    pub fn pause(&mut self)  {
        match self.sink.lock() {
            Ok(sink) => sink.pause(),
            Err(e) => eprintln!("Sink lock:\n{:?}", e)
        }
    }

    pub fn volume(&mut self) -> f32 {
        match self.sink.lock() {
            Ok(sink) => sink.volume(),
            Err(e) => {
                eprintln!("Sink lock:\n{:?}", e);
                0_f32
            }
        }
    }

    pub fn set_volume(&mut self, volume: f32)  {
        match self.sink.lock() {
            Ok(sink) => sink.set_volume(volume),
            Err(e) => eprintln!("Sink lock:\n{:?}", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohdio() {
        let mut player = match Player::start("https://rcavmedias-vh.akamaihd.net/i/51225e9a-2402-48db-9e39-47c65761b140/secured/2021-06-27_16_00_00_cestsibon_0000_,64,128,.mp4.csmil/master.m3u8?hdnea=st=1633221324~exp=1633221444~acl=/i/51225e9a-2402-48db-9e39-47c65761b140/secured/2021-06-27_16_00_00_cestsibon_0000_*~hmac=5dd3896f295e02382fb27f168962c54dd29186cfdaf739eb11f9e7e10f089f9a") {
            Ok(player) => player,
            Err(e) => {
                println!("{:?}", e);
                return assert!(false);
            }
        };

        thread::sleep(time::Duration::from_secs(30));
        player.pause();
        thread::sleep(time::Duration::from_secs(3));
        player.play().unwrap();
        thread::sleep(time::Duration::from_secs(3));
        player.set_volume(1.2);
        assert_eq!(player.volume(), 1.2);
        thread::sleep(time::Duration::from_secs(3));
        player.stop();
        assert!(true);
    }
}
