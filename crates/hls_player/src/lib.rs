use std::sync::mpsc::Receiver;
use anyhow::{anyhow, Context, Result};
use hls_handler::{start, Message};

enum State {
    Started,
    Stopped,
    Paused,
    Playing,
}

pub struct Player {
    rx : Option<Receiver<Message>>,
    state: State,
}

impl Player {
    pub fn new() -> Self {
        Self { rx: None, state : State::Stopped}
    }

    pub fn start(url: &str) -> Result<Result<Receiver<Message>>> {
        
    }
}
