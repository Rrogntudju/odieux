use anyhow::{Context, Result};
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};
use std::sync::mpsc::Receiver;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::{thread, time};

pub struct RxCursor {
    inner: Arc<Mutex<Vec<u8>>>,
    pos: u64,
    stop_signal: Arc<AtomicBool>,
}

impl RxCursor {
    pub fn new(rx: Receiver<Result<Box<Vec<u8>>>>) -> Result<Self> {
        let mut buf: Vec<u8> = Vec::new();
        let mut stream = *rx.recv()??;  // Wait for first TS packet
        buf.append(&mut stream);
        let inner = Arc::new(Mutex::new(buf));
        let inner2 = inner.clone();
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal2 = stop_signal.clone();

        thread::spawn(move || {
            while !stop_signal2.load(Ordering::Relaxed) {
                match rx.recv() {
                    Ok(message) => {
                        match message.context("Échec: réception du message") {
                            Ok(mut stream) =>  inner2.lock().expect("Poisoned lock").append(&mut stream),
                            Err(e) => return eprintln!("{:?}", e),
                        };
                    }
                    Err(_) => return, // tx was dropped
                }

                thread::sleep(time::Duration::from_millis(1000));
            }
        });

        Ok(Self { inner, pos: 0, stop_signal })
    }
}

impl Drop for RxCursor {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

impl Read for RxCursor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let inner = self.inner.lock().expect("Poisoned lock");
        let len = self.pos.min(inner.len() as u64);
        let n = Read::read(&mut &inner[(len as usize)..], buf)?;
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for RxCursor {
    fn seek(&mut self, style: SeekFrom) -> Result<u64, std::io::Error> {
        let (base_pos, offset) = match style {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }

            SeekFrom::End(n) => ((self.inner.lock().expect("Poisoned lock")).len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };
        let new_pos = if offset >= 0 {
            base_pos.checked_add(offset as u64)
        } else {
            base_pos.checked_sub((offset.wrapping_neg()) as u64)
        };
        match new_pos {
            Some(n) => {
                self.pos = n;
                Ok(self.pos)
            }
            None => Err(Error::new(ErrorKind::InvalidInput, "invalid seek to a negative or overflowing position")),
        }
    }
}
