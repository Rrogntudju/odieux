use std::sync::{Arc, Mutex};
use anyhow::{anyhow, Context, Result};
use hls_handler::{start, Message};
use rodio::Sink;


enum State {
    Started,
    Stopped,
    Paused,
    Playing,
}

pub struct Player {
    state: State,
    end_signal: Option<Arc<Mutex<bool>>>,
    sink: Option<Arc<Mutex<Sink>>>,
}

impl Player {
    pub fn new() -> Self {
        Self {state : State::Stopped, end_signal: None, sink: None}
    }

    pub fn start(&mut self, url: &str) -> Result<()> {

    }

    pub fn play(&mut self)  {
 
    }

    pub fn stop(&mut self) {

    }

    pub fn pause(&mut self) {

    }

}
/* if match end_signal.lock() {
    Ok(end) => *end,
    _ => return,
} {
    return;
} */