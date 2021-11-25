use gratte::gratte;
use serde_json::value::Value;
use std::env::args;
use std::error::Error;

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

    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
