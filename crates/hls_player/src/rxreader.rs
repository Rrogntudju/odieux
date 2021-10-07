use std::sync::mpsc::Receiver;
use anyhow::Result;
use std::io::Read;
struct RxReader {
    rx: Receiver<Result<Box<Vec<u8>>>>,
}

impl RxReader {
    fn new(rx: Receiver<Result<Box<Vec<u8>>>>) -> Self {
        Self { rx }
    }
}

impl Read for RxReader {
    fn read(&mut self, _: &mut [u8]) -> Result<usize, std::io::Error> { 
        todo!() }
}
    
