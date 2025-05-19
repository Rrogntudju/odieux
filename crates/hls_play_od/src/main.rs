use media::get_episodes;
use reqwest::Client;
use serde_json::Value;
use std::env::args;
use std::error::Error;
use std::time::Duration;

const TIME_OUT: u64 = 10;
const URL_VALIDEUR: &str =  "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";
const URL_VALIDEUR2: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls&manifestVersion=2";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let erreur = "Args: <url du programme> <page> <épisode>";
    let mut args = args();
    let prog = match args.nth(1) {
        Some(arg) => arg,
        None => return Err(erreur.into()),
    };

    let page = match args.next() {
        Some(arg) => arg,
        None => return Err(erreur.into()),
    };

    let num = match args.next() {
        Some(arg) => arg,
        None => return Err(erreur.into()),
    };

    let page = page.parse::<usize>()?;
    let épisodes = get_episodes(page, &prog).await?;

    let num = num.parse::<usize>()?.clamp(1, épisodes.len());
    let media_id = &épisodes[num - 1].media_id;
    if media_id.is_empty() {
        return Err("Aucune musique diffusée disponible".into());
    }
    let url = URL_VALIDEUR.replace("{}", media_id);
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let response = client.get(&url).send().await?.text().await?;
    let value: Value = serde_json::from_str(&response)?;

    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
