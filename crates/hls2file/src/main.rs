use std::env;
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

fn main() -> Result<(), Box<dyn Error>> {
    let master_url = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir un url master.m3u8 validé".into()),
    };

    let aac_filename = match args().nth(2) {
        Some(arg) => arg,
        None => return Err("Fournir un nom de fichier aac".into()),
    };

    let rx = hls_handler::start(&master_url)?;
    let mut aac = env::temp_dir();
    aac.push(aac_filename);

    let mut file = BufWriter::new(File::create(aac)?);

    for message in rx {
        match message {
            Ok(stream) => file.write_all(&stream)?,
            Err(e) => return Err(e.into()),
        };
    }
    
    file.flush()?;

    Ok(())
}
