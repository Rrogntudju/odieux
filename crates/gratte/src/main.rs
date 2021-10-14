use serde::Serialize;
use std::default::Default;
use std::env;
use std::error::Error;
use std::fs;
use Soup;

#[derive(Serialize)]
struct Emission {
    titre: String,
    url: String,
}

#[derive(Serialize, Default)]
struct Emissions(Vec<Emission>);

fn gratte(url: &str, out: &str) -> Result<(), Box<dyn Error>> {
    let mut émissions = Emissions::default();
    for i in 1.. {
        let response = match minreq::get(url).with_timeout(10).send() {
            Ok(response) => match response.status_code {
                200 => response,
                403 => break,
                _ => return Err(format!("{} a retourné {}", url, response.reason_phrase).into()),
            },
            Err(e) => return Err(e.into()),
        };
        let soup = Soup::new(response);
    }
    let mut json = env::temp_dir();
    json.push(out);
    fs::write(json, serde_json::to_string(&émissions)?)?;
    
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    gratte(
        "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber={}",
        "csb.json",
    )?;
    Ok(())
}
