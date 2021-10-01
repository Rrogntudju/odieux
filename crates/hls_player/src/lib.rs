use anyhow::{anyhow, Result};
use hls_handler;
use rodio::{OutputStream, Sink};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;

fn handle_sink(sink: Arc<Mutex<Sink>>, tx: Receiver<Result<Box<Vec<u8>>>>, end_signal: Arc<Mutex<bool>>) {
    loop {
        match end_signal.lock() {
            Ok(end) if *end => return,
            _ => return,
        }
    }
}

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
        thread::spawn(move || handle_sink(sink2, tx, end_signal2));

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
