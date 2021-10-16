use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use soup::prelude::*;
use std::default::Default;

#[derive(Serialize, Deserialize, Default)]
struct Episode {
    titre: String,
    media_id: String,
}

#[derive(Serialize, Default)]
struct Episodes(Vec<Episode>);

pub fn gratte(url: &str, page: u16) -> Result<String> {
    let mut épisodes = Episodes::default();
    let url = format!("{}{}", url, page);
    let page = match minreq::get(&url).with_timeout(10).send() {
        Ok(response) => match response.status_code {
            200 => response,
            403 => return Err(anyhow!("Page {} inexistante", page)),
            _ => return Err(anyhow!("{} a retourné {}", url, response.reason_phrase)),
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
        Some(s) => serde_json::from_str(s.trim_start_matches("window._rcState_ = /*bns*/ ").trim_end_matches(" /*bne*/;"))?,
        None => return Err(anyhow!("script introuvable")),
    };
    let items = &valeur["pagesV2"]["pages"][url.trim_start_matches("https://ici.radio-canada.ca")]["data"]["content"]["contentDetail"]["items"];
    match items {
        items if items.is_array() => {
            for j in 0.. {
                match &items[j] {
                    item if item.is_object() => {
                        let item_id = &item["playlistItemId"];
                        let épisode = json!({
                            "titre": &item_id["title"],
                            "media_id": &item_id["mediaId"]
                        });
                        épisodes.0.push(serde_json::from_value(épisode).unwrap_or_default());
                    }
                    _ => break,
                }
            }
        }
        _ => return Err(anyhow!("items inexistant")),
    }
    Ok(serde_json::to_string(&épisodes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csb() {
        match gratte("https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=", 1) {
            Ok(json) => assert_ne!(json, "[]"),
            Err(e) => {
                println!("{:?}", e);
                assert!(false);
            }
        }
    }
}
