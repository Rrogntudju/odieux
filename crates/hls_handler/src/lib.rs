use anyhow::{anyhow, Context, Result};
use decrypt_aes128::decrypt_aes128;
use hls_m3u8::tags::VariantStream;
use hls_m3u8::{MasterPlaylist, MediaPlaylist, Decryptable};
use hls_m3u8::types::EncryptionMethod;
use minreq;
use mpeg2ts::ts::payload::Bytes;
use std::collections::HashMap;
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
        Ok(response) => response.as_str().unwrap_or_default(),
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let master = match MasterPlaylist::try_from(response).context("Validation de MasterPlayList") {
        Ok(master) => master,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    // Select the audio stream with maximum bitrate
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
        Ok(response) => response.as_str().unwrap_or_default(),
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let media = match MediaPlaylist::try_from(response).context("Validation de MediaPlayList") {
        Ok(media) => media,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let mut cache: HashMap<String, Vec<u8>> = HashMap::new(); 
    
    for (_, media_segment) in media.segments {
        let segment_response = match minreq::get(media_segment.uri().as_ref())
            .with_timeout(TIME_OUT)
            .send()
            .context(format!("get {}", media_segment.uri().as_ref()))
        {
            Ok(response) => response.as_bytes(),
            Err(e) => {
                tx.send(Err(e)).unwrap_or_default();
                return;
            }
        };

        let keys = media_segment.keys();

        let decrypted = if keys.is_empty() {
            segment_response.to_owned()
        } else {
            let key = keys.iter().find(|k| k.method == EncryptionMethod::Aes128);
            
            let (uri, iv) = match key {
                Some(key) => (key.uri().as_ref().to_string(), key.iv.to_slice()),
                None => {
                    tx.send(Err(anyhow!("Le segment n'est pas chiffré avec AES-128"))).unwrap_or_default();
                    return;
                }
            };

            let key = match cache.get(&uri) {
                Some(key) => key.to_owned(),
                None => {
                    match minreq::get(&uri)
                        .with_timeout(TIME_OUT)
                        .send()
                        .context(format!("get {}", &uri))
                    {
                        Ok(response) => { 
                            let response = response.into_bytes(); 
                            cache.insert(uri.clone(), response.clone()); 
                            response
                        }
                        Err(e) => {
                            tx.send(Err(e)).unwrap_or_default();
                            return;
                        }
                    }
                }
            };
            
            let iv = match iv {
                Some(iv) => iv,
                None => {
                    tx.send(Err(anyhow!("Initialization Vector manquant"))).unwrap_or_default();
                    return;
                }
            };

            match decrypt_aes128(&key, &iv, segment_response) {
                Ok(s) => s,
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            }
        };
    };
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
