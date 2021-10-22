use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use soup::prelude::*;
use std::default::Default;

#[derive(Serialize, Default, Clone)]
#[allow(dead_code)]
pub struct Episode {
    titre: String,
    media_id: String,
}

pub fn gratte(url: &str, page: usize) -> Result<Vec<Episode>> {
    let mut épisodes = Vec::new();
    let url = format!("{}{}", url, page);
    let page = minreq::get(&url).with_timeout(10).send()?;
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
                        épisodes.push(Episode {
                            titre: item_id["title"].to_string(),
                            media_id: item_id["mediaId"].to_string(),
                        });
                    }
                    _ => break,
                }
            }
        }
        _ => return Err(anyhow!("items inexistant")),
    }
    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csb() {
        match gratte("https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=", 1) {
            Ok(épisodes) => assert!(épisodes.len() > 0),
            Err(e) => {
                println!("{:?}", e);
                assert!(false);
            }
        }
    }
}
