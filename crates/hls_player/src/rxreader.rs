use std::sync::mpsc::Receiver;
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};

pub struct RxReader {
    rx: Receiver<Result<Box<Vec<u8>>>>,
}

impl RxReader {
    pub fn new(rx: Receiver<Result<Box<Vec<u8>>>>) -> Self {
        Self { rx }
    }
}

impl Read for RxReader {
    fn read(&mut self, _: &mut [u8]) -> Result<usize, std::io::Error> { 
        todo!() }
}

impl Seek for RxReader {
    fn seek(&mut self, _: SeekFrom) -> Result<u64, std::io::Error> { 
        todo!() }
}
    
