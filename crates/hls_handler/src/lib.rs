use anyhow::{Context, Error, Result, anyhow, bail};
use hls_m3u8::tags::VariantStream;
use hls_m3u8::types::EncryptionMethod;
use hls_m3u8::{Decryptable, MasterPlaylist, MediaPlaylist};
use mpeg2ts::ts::{Pid, ReadTsPacket, TsPacketReader, TsPayload};
use reqwest::Client;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;
use std::time::{Duration, Instant};
use url::{ParseError, Url};

enum InitState {
    Pid0,
    Pmt(Pid),
}

type Message = Result<Vec<u8>>;

const TIME_OUT: u64 = 30;
const MAX_RETRIES: usize = 20;
const RETRY_DELAY: u64 = 250;
const BOUND: usize = 3;

fn decrypt_aes128(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let cipher = libaes::Cipher::new_128(key.try_into().context("La clé n'a pas une longueur de 16 bytes")?);
    let decrypted = cipher.cbc_decrypt(iv, data);
    match decrypted.is_empty() {
        false => Ok(decrypted),
        true => Err(anyhow!("La décryption a échoué")),
    }
}

fn base_or_join(base: &Url, url: &str) -> Result<Url> {
    match Url::parse(url) {
        Ok(url) => Ok(url),
        Err(ParseError::RelativeUrlWithoutBase) => base.join(url).context(format!("Échec: join de l'url {}", url)),
        Err(e) => Err(e.into()),
    }
}

async fn get(url: &str, client: &Client) -> Result<Vec<u8>> {
    let mut retries = 0;
    loop {
        match client.get(url).send().await {
            Ok(response) => break Ok(response.bytes().await?.to_vec()),
            Err(e) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    bail!("get {url} a échoué après {MAX_RETRIES} tentatives");
                } else {
                    eprintln!("{:#}", Error::new(e).context(format!("Échec: get {url}")));
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY)).await;
                    continue;
                }
            }
        }
    }
}

// Le segment est un fichier MPEG-TS encrypté qui contient du AAC
async fn hls_on_demand1(media_url: Url, client: Client, tx: SyncSender<Message>) {
    let response = match get(media_url.as_str(), &client).await {
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
        let segment_url = match base_or_join(&media_url, media_segment.uri()).context("Échec: base_or_join de l'url media segment") {
            Ok(url) => url,
            Err(e) => {
                tx.send(Err(e)).unwrap_or_default();
                return;
            }
        };
        let segment_response = match get(segment_url.as_str(), &client).await {
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
                Some(key) => (key.uri().as_ref(), key.iv.to_slice()),
                None => {
                    tx.send(Err(anyhow!("Le segment n'est pas chiffré avec AES-128"))).unwrap_or_default();
                    return;
                }
            };

            let key = match cache.get(uri) {
                Some(key) => key,
                None => match get(uri, &client).await {
                    Ok(response) => {
                        cache.insert(uri.to_owned(), response);
                        cache.get(uri).unwrap()
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

            match decrypt_aes128(key, &iv, &segment_response) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            }
        };

        let mut ts = TsPacketReader::new(decrypted.as_slice());

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
                                TsPayload::Pmt(pmt) => break pmt.es_info[0].elementary_pid,
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

// Le segment est un fichier AAC encrypté
async fn hls_on_demand2(media_url: Url, client: Client, tx: SyncSender<Message>) {
    let response = match get(media_url.as_str(), &client).await {
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
    let mut prec_uri = String::new(); // Problème d'URIs identiques

    for (_, media_segment) in media.segments {
        if prec_uri == media_segment.uri().as_ref() {
            continue; // Avec un media correctement construit, on n'aboutit jamais ici...
        } else {
            prec_uri = media_segment.uri().to_string();
        }

        let segment_url = match base_or_join(&media_url, media_segment.uri()).context("Échec: base_or_join de l'url media segment") {
            Ok(url) => url,
            Err(e) => {
                tx.send(Err(e)).unwrap_or_default();
                return;
            }
        };
        let segment_response = match get(segment_url.as_str(), &client).await {
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
                Some(key) => (key.uri().as_ref(), key.iv.to_slice()),
                None => {
                    tx.send(Err(anyhow!("Le segment n'est pas chiffré avec AES-128"))).unwrap_or_default();
                    return;
                }
            };

            let key = match cache.get(uri) {
                Some(key) => key,
                None => match get(uri, &client).await {
                    Ok(response) => {
                        cache.insert(uri.to_owned(), response);
                        cache.get(uri).unwrap()
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

            match decrypt_aes128(key, &iv, &segment_response) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    tx.send(Err(e)).unwrap_or_default();
                    return;
                }
            }
        };

        if tx.send(Ok(decrypted)).is_err() {
            return; // rx was dropped
        }
    }
}

async fn hls_live(media_url: Url, client: Client, tx: SyncSender<Message>) {
    let mut sequence = String::new();
    loop {
        let start = Instant::now();
        let mut changed = false;

        let response = match get(media_url.as_str(), &client).await {
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

        for (_, media_segment) in media.segments {
            let uri = media_segment.uri().as_ref();
            if sequence.as_str() < uri {
                let segment_url = match base_or_join(&media_url, uri).context("Échec: base_or_join de l'url media segment") {
                    Ok(url) => url,
                    Err(e) => {
                        tx.send(Err(e)).unwrap_or_default();
                        return;
                    }
                };
                let segment_response = match get(segment_url.as_str(), &client).await {
                    Ok(response) => response,
                    Err(e) => {
                        tx.send(Err(e)).unwrap_or_default();
                        return;
                    }
                };
                if tx.send(Ok(segment_response)).is_err() {
                    return; // rx was dropped
                }
                changed = true;
                sequence = uri.to_owned();
            }
        }
        let delay = match changed {
            true => media.target_duration.saturating_sub(start.elapsed()),
            false => media.target_duration / 2,
        };
        thread::sleep(delay);
    }
}

async fn handle_hls(master_url: Url, client: Client, tx: SyncSender<Message>) {
    let response = match get(master_url.as_str(), &client).await {
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

    // Selectionner le flux mp4a.40.2 (AAC-LC) ayant le «bitrate» le plus élevé
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
            tx.send(Err(anyhow!("Pas de stream mp4a.40.2 dans {}", master_url.as_str())))
                .unwrap_or_default();
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

    let url = match base_or_join(&master_url, media_url).context("Échec: base_or_join de l'url MediaPlaylist") {
        Ok(url) => url,
        Err(e) => {
            tx.send(Err(e)).unwrap_or_default();
            return;
        }
    };

    if master.has_independent_segments {
        hls_on_demand2(url, client, tx).await;
    } else if media_url.starts_with("https://rcavliveaudio.akamaized.net") {
        hls_live(url, client, tx).await
    } else {
        hls_on_demand1(url, client, tx).await;
    }
}

pub fn start(url: &str) -> Result<Receiver<Message>> {
    let master_url = Url::try_from(url).context("Échec: validation de l'url MasterPlaylist")?;
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let (tx, rx) = sync_channel::<Message>(BOUND);
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(handle_hls(master_url, client, tx));
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::{decrypt_aes128, start};

    #[test]
    fn ohdio() {
        let rx = start("Insérer un url master.m3u8 Ohdio validé").unwrap();
        match rx.recv() {
            Ok(s) => match s {
                Ok(message) => assert!(message.len() > 0),
                Err(e) => {
                    println!("{e:?}");
                    assert!(false);
                }
            },
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }

    #[test]
    fn test_decrypt() {
        let key = "4567890123456789".as_bytes();
        let iv = "1234567890123456".as_bytes();
        let data = [
            0xDA, 0x52, 0xF9, 0x7B, 0xAB, 0xAE, 0x0A, 0x79, 0x7F, 0x1C, 0x11, 0xEC, 0xB2, 0x09, 0x9F, 0xB0,
        ];

        let result = decrypt_aes128(&key, &iv, &data).unwrap();
        assert_eq!(String::from_utf8(result).unwrap(), "DOH!");
    }
}
