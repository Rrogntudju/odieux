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

type Message = Result<Vec<u8>>;
const TIME_OUT: u64 = 10;
const BOUND: usize = 3;

fn get(url: &str) -> Result<Vec<u8>> {
    Ok(minreq::get(url)
        .with_timeout(TIME_OUT)
        .send()
        .with_context(|| format!("Échec: get {}", url))?
        .into_bytes())
}

fn handle_hls(url: Url, tx: SyncSender<Message>) {
    let response = match get(url.as_str()) {
        Ok(response) => String::from_utf8(response).unwrap_or_default(),
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    let master = match MasterPlaylist::try_from(response.as_str()).context("Échec: validation de MasterPlayList") {
        Ok(master) => master,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    // Select the mp4a.40.2 audio stream with the highest bitrate
    let vs = match master
        .variant_streams
        .iter()
        .filter(|vs| match vs.codecs() {
            Some(codecs) if codecs.len() == 1 => codecs[0] == "mp4a.40.2",
            _ => false,
        })
        .max_by_key(|vs| vs.bandwidth())
    {
        Some(vs) => vs,
        None => {
            tx.send(Err(anyhow!("Pas de stream mp4a.40.2 dans {}", url.as_str()))).unwrap_or_default();
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

    let media = match MediaPlaylist::try_from(response.as_str()).context("Échec: validation de MediaPlayList") {
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
                None => match get(&uri) {
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
            let packet = match ts.read_ts_packet().context("Échec: lecture d'un paquet TS") {
                Ok(packet) => {
                    match packet {
                        Some(packet) => packet,
                        None => break, // End of packets
                    }
                }
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            };

            // Assuming just one AAC stream with pid 256
            if packet.header.pid.as_u16() == 256 {
                let data = match packet.payload {
                    Some(payload) => match payload {
                        TsPayload::Pes(pes) => pes.data,
                        TsPayload::Raw(data) => data,
                        _ => continue,
                    },
                    None => {
                        tx.send(Err(anyhow!("Pas de payload"))).unwrap_or_default();
                        return;
                    }
                };

                stream.extend_from_slice(&data[..]);
            }
        }

        if tx.send(Ok(stream)).is_err() {
            return; // rx was dropped
        }
    }
}

pub fn start(url: &str) -> Result<Receiver<Message>> {
    let master_url = Url::try_from(url).context("Échec: validation de l'url MasterPlaylist")?;
    let (tx, rx) = sync_channel::<Message>(BOUND);
    thread::spawn(move || handle_hls(master_url, tx));

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::start;

    #[test]
    fn ohdio() {
        let rx = start("Insérer un url master.m3u8 Ohdio validé").unwrap();
        match rx.recv() {
            Ok(s) => match s {
                Ok(message) => assert!(message.len() > 0),
                Err(e) => {
                    println!("{:?}", e);
                    assert!(false);
                }
            },
            Err(e) => {
                println!("{:?}", e);
                assert!(false);
            }
        }
    }
}
