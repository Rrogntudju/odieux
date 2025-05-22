use media::{get_episodes, get_media_id};
use reqwest::Client;
use serde_json::Value;
use std::env::args;
use std::error::Error;
use std::time::Duration;

const TIME_OUT: u64 = 10;
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls&manifestVersion=2";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let erreur = "Args: <id du programme> <page> <no de l'episode>";
    let mut args = args();
    let prog = match args.nth(1) {
        Some(arg) => arg.parse::<usize>().unwrap_or_default(),
        None => return Err(erreur.into()),
    };

    let page = match args.next() {
        Some(arg) => arg.parse::<usize>().unwrap_or_default(),
        None => return Err(erreur.into()),
    };

    let episode_no = match args.next() {
        Some(arg) => arg,
        None => return Err(erreur.into()),
    };

    let episodes = get_episodes(prog, page).await?;
    let no = episode_no.parse::<usize>()?.clamp(1, episodes.len()) - 1;
    let media_id = get_media_id(&episodes[no].id).await?;

    let url = URL_VALIDEUR.replace("{}", &media_id);
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let response = client.get(&url).send().await?.text().await?;
    let value: Value = serde_json::from_str(&response)?;

    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
