use anyhow::{anyhow, Context, Result};
use hls_handler;
use rodio::{OutputStream, Sink};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

enum State {
    Started,
    Stopped,
    Paused,
}

fn handle_sink(sink: Arc<Mutex<Sink>>, tx: Receiver<Result<Box<Vec<u8>>>>, end_signal: Arc<Mutex<bool>>) {}

pub struct Player {
    state: State,
    end_signal: Option<Arc<Mutex<bool>>>,
    sink: Option<Arc<Mutex<Sink>>>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            state: State::Stopped,
            end_signal: None,
            sink: None,
        }
    }

    pub fn start(&mut self, url: &str) -> Result<()> {
        let tx = hls_handler::start(url)?;
        let (_, stream_handle) = OutputStream::try_default()?;
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle)?));
        let sink2 = sink.clone();
        self.sink = Some(sink);

        Ok(())
    }

    pub fn play(&mut self) {}

    pub fn stop(&mut self) {}

    pub fn pause(&mut self) {}
}
/* if match end_signal.lock() {
    Ok(end) => *end,
    _ => return,
} {
    return;
} */
