use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::{thread, time};

pub struct Player {
    _output_stream: OutputStream,
    sink: Arc<Mutex<Sink>>,
    stop_signal: Arc<AtomicBool>,
}

impl Player {
    pub fn start(url: &str) -> Result<Self> {
        let rx = hls_handler::start(url)?;
        let (_output_stream, stream_handle) = OutputStream::try_default().context("Échec: création de OutputStream")?;
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

        Ok(Self { _output_stream, sink, stop_signal })
    }

    pub fn play(&mut self) {
        match self.sink.lock() {
            Ok(sink) => sink.play(),
            Err(e) => return eprintln!("Sink lock:\n{:?}", e),
        }
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
