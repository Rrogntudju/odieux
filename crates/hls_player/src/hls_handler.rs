use anyhow::{anyhow, Context, Result};
use decrypt_aes128::decrypt_aes128;
use hls_m3u8::{MasterPlaylist, MediaPlaylist};
use minreq;
use mpeg2ts::ts::payload::Bytes;
use std::convert::TryFrom;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;
use url::Url;

type Message = Result<Box<Vec<u8>>>;
const TIME_OUT: u64 = 10;
const BOUND: usize = 2;

fn handle_hls(url: Url, tx: SyncSender<Message>) {
    let response = match minreq::get(url).with_timeout(TIME_OUT).send().context("Téléchargement de MasterPlayList") {
        Ok(response) => response,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let masterPl = match MasterPlaylist::try_from(response.as_str().unwrap_or_default()).context("Validation de MasterPlayList") {
        Ok(mpl) => mpl,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let vs = masterPl.audio_streams().max_by_key(|vs| vs.bandwidth());  // Select stream with maximum bitrate
}

pub fn start(url: &str) -> Result<Receiver<Message>> {
    let master_url = Url::try_from(url).context("Validation de l'url MasterPlaylist")?;
    let (tx, rx) = sync_channel::<Message>(BOUND);
    thread::spawn(move || handle_hls(master_url, tx));

    Ok(rx)
}
