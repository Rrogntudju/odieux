use serde_json::value::Value;
use std::error::Error;

const TIME_OUT: u64 = 10;
const URL_VALIDEUR: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls";

fn main() -> Result<(), Box<dyn Error>> {
    let value: Value = minreq::get(URL_VALIDEUR).with_timeout(TIME_OUT).send()?.json()?;
    let (sink, _output_stream) = hls_player::start(value["url"].as_str().unwrap_or_default())?;
    sink.sleep_until_end();

    Ok(())
}
