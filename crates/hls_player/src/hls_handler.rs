use hls_m3u8::{MasterPlaylist, MediaPlaylist,};
use mpeg2ts::ts::payload::Bytes;
use std::collections::VecDeque;
use std::convert::TryFrom;
use url::Url;
use anyhow::{Result, Context, Error};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

struct HlsHandler {
    // masterPlayList: MasterPlaylist<'a>,
    // mediaPlayList: MediaPlaylist<'a>,
   // streams: VecDeque<Bytes>,
   sender: Option<SyncSender<Box<Vec<u8>>>>
}

impl HlsHandler {
    fn new() -> Self {

        HlsHandler { sender: None }
    }

    fn start(&mut self, url: &str) -> Result<Receiver<Box<Vec<u8>>> {
        let master_url = Url::try_from(url)?.

    }
}