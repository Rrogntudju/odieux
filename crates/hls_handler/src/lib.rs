use anyhow::{anyhow, Context, Result};
use decrypt_aes128::decrypt_aes128;
use hls_m3u8::tags::VariantStream;
use hls_m3u8::types::EncryptionMethod;
use hls_m3u8::{Decryptable, MasterPlaylist, MediaPlaylist};
use mpeg2ts::ts::{ReadTsPacket, TsPacketReader, TsPayload};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;
use url::Url;

type Message = Result<Box<Vec<u8>>>;
const TIME_OUT: u64 = 10;
const BOUND: usize = 3;

fn get(url: &str) -> Result<Vec<u8>> {
    match minreq::get(url)
        .with_timeout(TIME_OUT)
        .send()
        .context(format!("get {}", url))
    {
        Ok(response) => {
            if response.status_code == 200 {
                Ok(response.into_bytes())
            } else {
                Err(anyhow!("{} a retourné {}", url, response.reason_phrase))
            }
        }
        Err(e) => Err(e),
    }
}

fn handle_hls(url: Url, tx: SyncSender<Message>) {
    let response = match get(url.as_str()) {
        Ok(response) => String::from_utf8(response).unwrap_or_default(),
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let master = match MasterPlaylist::try_from(response.as_str()).context("Validation de MasterPlayList") {
        Ok(master) => master,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    // Select the audio stream with the highest bitrate
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

    let response = match get(media_url.as_ref()) {
        Ok(response) => String::from_utf8(response).unwrap_or_default(),
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let media = match MediaPlaylist::try_from(response.as_str()).context("Validation de MediaPlayList") {
        Ok(media) => media,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let mut cache: HashMap<String, Vec<u8>> = HashMap::new();

    for (_, media_segment) in media.segments {
        let segment_response = match get(media_segment.uri().as_ref()) {
            Ok(response) => response,
            Err(e) => {
                tx.send(Err(e)).unwrap_or_default();
                return;
            }
        };

        let keys = media_segment.keys();

        let decrypted = if keys.is_empty() {
            segment_response
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
                None => match get(&uri).context(format!("get {}", &uri)) {
                    Ok(response) => {
                        cache.insert(uri, response.clone());
                        response
                    }
                    Err(e) => {
                        tx.send(Err(e)).unwrap_or_default();
                        return;
                    }
                },
            };

            let iv = match iv {
                Some(iv) => iv,
                None => {
                    tx.send(Err(anyhow!("Initialization Vector manquant"))).unwrap_or_default();
                    return;
                }
            };

            match decrypt_aes128(&key, &iv, &segment_response) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            }
        };

        let mut ts = TsPacketReader::new(decrypted.as_slice());
        let mut stream: Vec<u8> = Vec::new();
        loop {
            let packet = match ts.read_ts_packet().context("Lecture d'un paquet TS") {
                Ok(packet) => {
                    match packet {
                        Some(packet) => packet,
                        None => break, // End of packets
                    }
                }
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    break;
                }
            };

            let pes = match packet.payload {
                Some(payload) => match payload {
                    TsPayload::Pes(pes) => pes,
                    _ => {
                        tx.send(Err(anyhow!("Pas de paquet PES"))).unwrap_or_default();
                        return;
                    }
                },
                None => {
                    tx.send(Err(anyhow!("Pas de payload"))).unwrap_or_default();
                    return;
                }
            };

            stream.copy_from_slice(&pes.data[..]);
        }

        tx.send(Ok(Box::new(stream))).unwrap_or_default();
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
    use super::*;

    #[test]
    fn ohdio() {
        let rx = start("https://rcavmedias-vh.akamaihd.net/i/51225e9a-2402-48db-9e39-47c65761b140/secured/2021-06-27_16_00_00_cestsibon_0000_,64,128,.mp4.csmil/master.m3u8?hdnea=st=1632850095~exp=1632850215~acl=/i/51225e9a-2402-48db-9e39-47c65761b140/secured/2021-06-27_16_00_00_cestsibon_0000_*~hmac=368cfd88720986250b75e4ee1c6edf621bb018eceab4ea51a4e9e90178602bb5").unwrap();
        match rx.recv() {
            Ok(s) => match s {
                Ok(_) => assert!(true),
                Err(e) => {
                    println!("{:?}", e);
                    assert!(false);
                }
            },
            Err(e) => {
                println!("{:?}", e);
                assert!(true);
            }
        }
    }
}
