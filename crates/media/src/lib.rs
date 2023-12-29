use anyhow::{bail, ensure, Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;
use std::time::Duration;

const TIME_OUT: u64 = 30;

#[derive(Deserialize, Serialize, Default, Clone, PartialEq)]
pub struct Episode {
    pub titre: String,
    pub media_id: String,
}

pub async fn get_episodes(no: usize, url: &str) -> Result<Vec<Episode>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let url = url.replace("{}", &format!("{no}"));
    let page = match client.get(&url).send().await {
        Ok(response) => response.text().await?,
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                bail!("La page {no} n'existe pas");
            } else {
                bail!(e);
            }
        }
    };
    let valeur: Value = serde_json::from_str(&page)?;
    let items = valeur["content"]["contentDetail"]["items"]
        .as_array()
        .context("items n'est pas un array")?;
    let mut épisodes = Vec::new();
    for item in items {
        ensure!(item.is_object(), "item n'est pas un objet");
        let media = &item["media2"];
        épisodes.push(Episode {
            titre: media["title"].as_str().unwrap_or_default().to_owned(),
            media_id: media["id"].as_str().unwrap_or_default().to_owned(),
        });
    }
    ensure!(!épisodes.is_empty(), "La page {no} n'existe pas");
    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSB: &str = "https://services.radio-canada.ca/neuro/sphere/v1/audio/apps/products/programmes-v2/cestsibon/{}?context=web&pageNumber={}";
    const TUM: &str = "https://services.radio-canada.ca/neuro/sphere/v1/audio/apps/products/programmes-v2/touteunemusique/{}?context=web&pageNumber={}";

    #[tokio::test]
    async fn csb() {
        match get_episodes(13, CSB).await {
            Ok(_) => assert!(true),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }

    #[tokio::test]
    async fn tum() {
        match get_episodes(38, TUM).await {
            Ok(_) => assert!(true),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }
}
