use std::sync::mpsc::Receiver;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom, Error, ErrorKind};

pub struct RxCursor {
    rx: Receiver<Result<Box<Vec<u8>>>>,
    inner: Vec<u8>,
    pos: u64
}

impl RxCursor {
    pub fn new(rx: Receiver<Result<Box<Vec<u8>>>>) -> Self {
        Self { rx, pos: 0 }
    }

    pub fn remaining_slice(&self) -> &[u8] {
        let len = self.pos.min((&self.inner).len() as u64);
        &(self.inner)[(len as usize)..]
    }
}

impl Read for RxCursor {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> { 
        let n = Read::read(&mut self.remaining_slice(), buf)?;
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
            SeekFrom::End(n) => ((&self.inner).len() as u64, n),
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
            None => Err(Error::new(ErrorKind::InvalidInput, "invalid seek to a negative or overflowing position"))
        } 
    }
}
    
