use hls_m3u8::{MasterPlaylist, MediaPlaylist,};
use mpeg2ts::ts::payload::Bytes;
use std::convert::TryFrom;
use url::Url;
use anyhow::{Result, Context};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;
use decrypt_aes128::decrypt_aes128;

 
fn handle_hls(url: Url, tx: SyncSender<Box<Vec<u8>>>) {

}

pub fn start(url: &str) -> Result<Receiver<Box<Vec<u8>>>> {
    let master_url = Url::try_from(url).context("Validation de l'url MasterPlaylist")?;
    let (tx, rx) = sync_channel::<Box<Vec<u8>>>(2);
    thread::spawn(move || handle_hls(master_url, tx));

    Ok(rx)
}
