use anyhow::{anyhow, Context, Result};
use decrypt_aes128::decrypt_aes128;
use hls_m3u8::tags::{VariantStream, ExtXKey};
use hls_m3u8::{MasterPlaylist, MediaPlaylist, Decryptable};
use hls_m3u8::types::EncryptionMethod;
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
    let response = match minreq::get(url.as_str())
        .with_timeout(TIME_OUT)
        .send()
        .context(format!("get {}", url.as_str()))
    {
        Ok(response) => response,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let master = match MasterPlaylist::try_from(response.as_str().unwrap_or_default()).context("Validation de MasterPlayList") {
        Ok(master) => master,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    // Select the stream with maximum bitrate
    let vs = match master.audio_streams().max_by_key(|vs| vs.bandwidth()) {
        Some(vs) => vs,
        None => {
            tx.send(Err(anyhow!("Pas de stream audio dans {}", url.as_str()))).unwrap_or_default();
            return;
        }
    };

    let media_url = match vs {
        VariantStream::ExtXStreamInf { uri, .. } => uri,
        _ => {
            tx.send(Err(anyhow!("DOH!: ExtXIFrame"))).unwrap_or_default();
            return;
        }
    };

    let response = match minreq::get(media_url.as_ref())
        .with_timeout(TIME_OUT)
        .send()
        .context(format!("get {}", media_url.as_ref()))
    {
        Ok(response) => response,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let media = match MediaPlaylist::try_from(response.as_str().unwrap_or_default()).context("Validation de MediaPlayList") {
        Ok(media) => media,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    for (_, segment) in media.segments {
        let response = match minreq::get(segment.uri().as_ref())
            .with_timeout(TIME_OUT)
            .send()
            .context(format!("get {}", segment.uri().as_ref()))
        {
            Ok(response) => response,
            Err(e) => {
                tx.send(Err(e)).unwrap_or_default();
                return;
            }
        };

        let keys = segment.keys();
        let decrypted = if keys.is_empty() {
            response.as_bytes()
        } else {
            let key = keys.iter().find(|k| k.method == EncryptionMethod::Aes128);
        };
    }
}

pub fn start(url: &str) -> Result<Receiver<Message>> {
    let master_url = Url::try_from(url).context("Validation de l'url MasterPlaylist")?;
    let (tx, rx) = sync_channel::<Message>(BOUND);
    thread::spawn(move || handle_hls(master_url, tx));

    Ok(rx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
