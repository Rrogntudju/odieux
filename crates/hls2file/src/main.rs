use gratte::gratte;
use serde_json::value::Value;
use std::env;
use std::env::args;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};

const TIME_OUT: u64 = 10;
const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";

fn main() -> Result<(), Box<dyn Error>> {
    let page = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de la page".into()),
    };

    let num = match args().nth(2) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de l'épisode".into()),
    };

    let page = page.parse::<usize>()?.clamp(1, 68);
    let épisodes = gratte(CSB, page)?;

    let num = num.parse::<usize>()?.clamp(1, épisodes.len());
    let url = URL_VALIDEUR.replace("{}", &épisodes[num - 1].media_id);
    let value: Value = minreq::get(&url).with_timeout(TIME_OUT).send()?.json()?;

    let mut aac = env::temp_dir();
    aac.set_file_name(&épisodes[num - 1].titre);
    aac.set_extension("aac");
    let mut file = BufWriter::new(File::create(aac)?);

    let rx = hls_handler::start(value["url"].as_str().unwrap_or_default())?;
    for message in rx {
        match message {
            Ok(stream) => file.write_all(&stream)?,
            Err(e) => return Err(e.into()),
        };
    }

    file.flush()?;

    Ok(())
}
