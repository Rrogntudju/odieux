use anyhow::{anyhow, Context, Result};
use hls_handler;
use rodio::{OutputStream, Sink};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;

fn handle_sink(sink: Arc<Mutex<Sink>>, tx: Receiver<Result<Box<Vec<u8>>>>, end_signal: Arc<Mutex<bool>>) {
    loop {
        match end_signal.lock() {
            Ok(end) if *end => return,
            _ => return
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
        thread::spawn(move || handle_sink(sink2, tx));

        Ok(Self { sink })
    }

    pub fn play(&mut self) {
        let sink = *self.sink.lock().expect("Poisoned lock");
        sink.play();
    }

    pub fn stop(&mut self) {
        let sink = *self.sink.lock().expect("Poisoned lock");
        sink.pause();
        drop(sink);
    }

    pub fn pause(&mut self) {
        let sink = *self.sink.lock().expect("Poisoned lock");
        sink.pause();
    }
}
