use anyhow::{anyhow, Context, Result};
use decrypt_aes128::decrypt_aes128;
use hls_m3u8::tags::VariantStream;
use hls_m3u8::types::EncryptionMethod;
use hls_m3u8::{Decryptable, MasterPlaylist, MediaPlaylist};
use mpeg2ts::ts::{Pid, ReadTsPacket, TsPacketReader, TsPayload};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread;
use url::{Url,ParseError};

enum InitState {
    Pid0,
    Pmt(Pid),
}
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

fn hls_on_demand(media_url: &str, tx: SyncSender<Message>) {
    let response = match get(media_url) {
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

        // Obtenir le pid du premier programme
        let mut state = InitState::Pid0;
        let program_pid = loop {
            let packet = match ts.read_ts_packet().context("Échec: lecture d'un paquet TS") {
                Ok(packet) => match packet {
                    Some(packet) => packet,
                    None => {
                        tx.send(Err(anyhow!("Fin prématurée des paquets"))).unwrap_or_default();
                        return;
                    }
                },
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            };

            match state {
                InitState::Pid0 => {
                    match packet.header.pid.as_u16() {
                        0 => match packet.payload {
                            Some(payload) => match payload {
                                TsPayload::Pat(pat) => {
                                    state = InitState::Pmt(pat.table[0].program_map_pid);
                                    continue;
                                }
                                _ => {
                                    tx.send(Err(anyhow!("Pas de PAT dans le PID 0"))).unwrap_or_default();
                                    return;
                                }
                            },
                            None => {
                                tx.send(Err(anyhow!("Pas de payload dans le PID 0"))).unwrap_or_default();
                                return;
                            }
                        },
                        1..=31 | 8191 => continue,
                        _ => {
                            tx.send(Err(anyhow!("Pas de PID 0"))).unwrap_or_default();
                            return;
                        }
                    };
                }
                InitState::Pmt(pid) => {
                    if packet.header.pid == pid {
                        match packet.payload {
                            Some(payload) => match payload {
                                TsPayload::Pmt(pmt) => break pmt.table[0].elementary_pid,
                                _ => {
                                    tx.send(Err(anyhow!("Pas de PMT dans le PID {}", pid.as_u16()))).unwrap_or_default();
                                    return;
                                }
                            },
                            None => {
                                tx.send(Err(anyhow!("Pas de payload dans le PID {}", pid.as_u16()))).unwrap_or_default();
                                return;
                            }
                        }
                    } else {
                        tx.send(Err(anyhow!("Pas de PID {}", pid.as_u16()))).unwrap_or_default();
                        return;
                    };
                }
            }
        };

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

            if packet.header.pid == program_pid {
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

fn hls_live(media_url: &str, tx: SyncSender<Message>) {

}

fn handle_hls(master_url: Url, tx: SyncSender<Message>) {
    let response = match get(master_url.as_str()) {
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

    // Select the mp4a.40.2 (AAC-LC) audio stream with the highest bitrate
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
            tx.send(Err(anyhow!("Pas de stream mp4a.40.2 dans {}", master_url.as_str()))).unwrap_or_default();
            return;
        }
    };

    let media_url = match vs {
        VariantStream::ExtXStreamInf { uri, .. } => uri,
        _ => {
            tx.send(Err(anyhow!("ExtXIFrameInf manquant"))).unwrap_or_default();
            return;
        }
    };

    match Url::try_from(media_url.as_ref()) {
        Ok(url) => hls_on_demand(url.as_str(), tx),
        Err(ParseError::RelativeUrlWithoutBase) => {
            match master_url.join(&media_url).context("Échec: conversion de l'url MediaPlaylist") {
                Ok(url) => hls_live(url.as_str(), tx),
                Err(e) => tx.send(Err(e)).unwrap_or_default(),
            }
            
        }
        Err(e) => tx.send(Err(anyhow!("{:?}\nÉchec: validation de l'url MediaPlaylist", e))).unwrap_or_default(),
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
