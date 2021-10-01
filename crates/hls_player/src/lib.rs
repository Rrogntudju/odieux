use anyhow::{anyhow, Result};
use hls_handler;
use rodio::{OutputStream, Sink};
use std::sync::{Arc, Mutex};
use std::{thread, time};

pub struct Player {
    sink: Arc<Mutex<Sink>>,
    end_signal: Arc<Mutex<bool>>,
}

impl Player {
    pub fn start(url: &str) -> Result<Self> {
        let tx = hls_handler::start(url)?;
        let (_, stream_handle) = OutputStream::try_default()?;
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle)?));
        let sink2 = sink.clone();
        let end_signal = Arc::new(Mutex::new(false));
        let end_signal2 = end_signal.clone();

        thread::spawn(move || {
            while !match end_signal2.lock() {
                Ok(end) => *end,
                Err(e) => {
                    eprintln!("End signal lock:\n{}", e);
                    return;
                }
            } {
                let sink = match sink2.lock() {
                    Ok(sink) => sink,
                    Err(e) => {
                        eprintln!("Sink lock:\n{}", e);
                        return;
                    }
                };

                while sink.len() < 3 {
                    match tx.recv() {}
                }

                thread::sleep(time::Duration::from_millis(1000));
            }
        });

        Ok(Self { sink, end_signal })
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

        match self.end_signal.lock() {
            Ok(mut end_signal) => *end_signal = true,
            Err(e) => return Err(anyhow!("End signal lock:\n{}", e)),
        }

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
