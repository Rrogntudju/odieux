use serde_json::value::Value;
use std::error::Error;
use std::time::Duration;
use reqwest::Client;

const TIME_OUT: u64 = 10;
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let response = client.get(URL_VALIDEUR).send().await?.text().await?;
    let value: Value = serde_json::from_str(&response)?;
    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
