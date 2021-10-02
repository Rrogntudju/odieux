use anyhow::{anyhow, Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, atomic::AtomicBool};
use std::{thread, time};
use std::io::Cursor;

pub struct Player {
    sink: Arc<Mutex<Sink>>,
    stop_signal: Arc<AtomicBool>,
}

impl Player {
    pub fn start(url: &str) -> Result<Self> {
        let rx = hls_handler::start(url)?;
        let (_, stream_handle) = OutputStream::try_default()?;
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle)?));
        let sink2 = sink.clone();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal2 = stop_signal.clone();

        thread::spawn(move || {
            while !stop_signal2.load(Ordering::Relaxed) {
                let sink = match sink2.lock() {
                    Ok(sink) => sink,
                    Err(e) => {
                        eprintln!("Sink lock:\n{}", e);
                        return;
                    }
                };

                if sink.len() < 2 {
                    match rx.recv() {
                        Ok(message) => {
                            let stream = match message {
                                Ok(stream) => stream,
                                Err(e) => {
                                    eprintln!("{}", e);
                                    return;
                                }
                            };
                            let source = match Decoder::new(Cursor::new(*stream)).context("Decoder") {
                                    Ok(source) => source,
                                    Err(e) => {
                                        eprintln!("{}", e);
                                        return;
                                    }
                            };
                            sink.append(source);
                        }
                        Err(_) => return // tx was dropped
                    }
                }
                drop(sink);
                
                thread::sleep(time::Duration::from_millis(1000));
            }
        });

        Ok(Self { sink, stop_signal })
    }

    pub fn play(&mut self) -> Result<()> {
        match self.sink.lock() {
            Ok(sink) => sink.play(),
            Err(e) => return Err(anyhow!("Sink lock:\n{}", e)),
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        match self.sink.lock() {
            Ok(sink) => sink.pause(),
            Err(e) => return Err(anyhow!("Sink lock:\n{}", e)),
        }

        self.stop_signal.store(true, Ordering::Relaxed);

        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        match self.sink.lock() {
            Ok(sink) => sink.pause(),
            Err(e) => return Err(anyhow!("Sink lock:\n{}", e)),
        }

        Ok(())
    }
}
