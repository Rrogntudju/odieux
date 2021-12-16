use gratte::gratte;
use reqwest::Client;
use serde_json::Value;
use std::env::args;
use std::error::Error;
use std::time::Duration;

const TIME_OUT: u64 = 10;
const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";
const PAGES: usize = 68;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let page = match args().nth(1) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de la page".into()),
    };

    let num = match args().nth(2) {
        Some(arg) => arg,
        None => return Err("Fournir le numéro de l'épisode".into()),
    };

    let page = page.parse::<usize>()?.clamp(1, PAGES);
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let épisodes = gratte(CSB, page, &client).await?;

    let num = num.parse::<usize>()?.clamp(1, épisodes.len());
    let media_id = &épisodes[num - 1].media_id;
    if media_id.is_empty() {
        return Err("Aucune musique diffusée disponible".into());
    }
    let url = URL_VALIDEUR.replace("{}", media_id);
    let response = client.get(&url).send().await?;
    let value: Value = serde_json::from_str(&response.text().await?)?;

    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
