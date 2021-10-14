use serde::Serialize;
use soup::prelude::*;
use std::default::Default;
use std::env;
use std::error::Error;
use std::fs;
use serde_json::Value;


#[derive(Serialize)]
struct Episode {
    titre: String,
    media_id: String,
}

#[derive(Serialize, Default)]
struct Episodes(Vec<Episode>);

fn gratte(url: &str, out: &str) -> Result<(), Box<dyn Error>> {
    let mut épisodes = Episodes::default();
    for i in 1.. {
        let url = format!("{}{}", url, i);
        let page = match minreq::get(&url).with_timeout(10).send() {
            Ok(response) => match response.status_code {
                200 => response,
                403 => break,
                _ => return Err(format!("{} a retourné {}", url, response.reason_phrase).into()),
            },
            Err(e) => return Err(e.into()),
        };
        let soup = Soup::new(page.as_str().unwrap_or("DOH!"));

        let script = soup
            .tag("script")
            .find_all()
            .filter_map(|s| match s.text() {
                t if t.starts_with("window._rcState_") => Some(t),
                _ => None,
            })
            .next();

        let valeur: Value = match script {
            Some(s) => serde_json::from_str(s.trim_start_matches("window._rcState_ = /*bns*/ "))?,
            None => return Err("script introuvable".into()),
        };

        let items = &valeur["pagesV2"]["pages"][url.trim_start_matches("https://ici.radio-canada.ca")]["data"]["content"]["contentDetail"]["items"];
    }
    let mut json = env::temp_dir();
    json.push(out);
    fs::write(json, serde_json::to_string(&épisodes)?)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    gratte(
        "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber={}",
        "csb.json",
    )?;
    Ok(())
}
