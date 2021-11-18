use std::env;
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use gratte::gratte;

const TIME_OUT: u64 = 10;
const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";

fn main() -> Result<(), Box<dyn Error>> {
    let page = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de la page".into()),
    };

    let numéro = match args().nth(2) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de l'émission".into()),
    };

    let aac_filename = match args().nth(3) {
        Some(arg) => arg,
        None => return Err("Fournir un nom de fichier aac".into()),
    };

    let émissions = gratte(CSB, page.parse::<usize>()?)?;
    let url = URL_VALIDEUR.replace("{}", &émissions[numéro.parse::<usize>()? + 1].media_id);
    let value = minreq::get(&url)
        .with_timeout(TIME_OUT)
        .send()?
        .json()?;
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
