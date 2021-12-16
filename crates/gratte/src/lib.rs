use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use soup::prelude::*;
use std::default::Default;

#[derive(Deserialize, Serialize, Default, Clone, PartialEq)]
pub struct Episode {
    pub titre: String,
    pub media_id: String,
}

pub async fn gratte(url: &str, page: usize, client: &Client) -> Result<Vec<Episode>> {
    let mut épisodes = Vec::new();
    let url = format!("{}{}", url, page);
    let page = client.get(&url).send().await?.text().await?;
    let soup = Soup::new(&page);
    let script = soup.tag("script").find_all().find_map(|s| match s.text() {
        t if t.starts_with("window._rcState_") => Some(t),
        _ => None,
    });
    let valeur: Value = match script {
        Some(s) => serde_json::from_str(s.trim_start_matches("window._rcState_ = /*bns*/ ").trim_end_matches(" /*bne*/;"))?,
        None => bail!("script introuvable"),
    };
    let items = &valeur["pagesV2"]["pages"][url.trim_start_matches("https://ici.radio-canada.ca")]["data"]["content"]["contentDetail"]["items"];
    match items {
        items if items.is_array() => {
            for j in 0.. {
                match &items[j] {
                    item if item.is_object() => {
                        let item_id = &item["playlistItemId"];
                        épisodes.push(Episode {
                            titre: item_id["title"].as_str().unwrap_or_default().replace("&nbsp;", " "),
                            media_id: item_id["mediaId"].as_str().unwrap_or_default().replace("&nbsp;", " "),
                        });
                    }
                    _ => break,
                }
            }
        }
        _ => bail!("items inexistant"),
    }
    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn csb() {
        let client = Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        match gratte(
            "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=",
            1,
            &client,
        )
        .await
        {
            Ok(épisodes) => assert!(épisodes.len() > 0),
            Err(e) => {
                println!("{:?}", e);
                assert!(false);
            }
        }
    }
}
