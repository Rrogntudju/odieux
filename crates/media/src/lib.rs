use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;
use std::time::Duration;
use urlencoding::encode;

const TIME_OUT: u64 = 30;
const GRAPHQL: &str = "https://services.radio-canada.ca/bff/audio/graphql";

#[derive(Deserialize, Serialize, Default, Clone, PartialEq)]
pub struct Episode {
    pub titre: String,
    pub id: String,
}

pub async fn get_episodes(prog_id: usize, page_no: usize) -> Result<Vec<Episode>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let params = format!(r##"opname=programmeById&variables={{"params":{{"context":"web","forceWithoutCueSheet":false,"id":{prog_id},"pageNumber":{page_no}}}"##);
    let url = [GRAPHQL, "?", &encode(&params)].concat();
    let page = match client.get(&url).header("Content-Type", "application/json").send().await {
        Ok(response) => response.text().await?,
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                bail!("La page {page_no} n'existe pas");
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
    ensure!(!épisodes.is_empty(), "La page {page_no} n'existe pas");
    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSB: &str = "https://services.radio-canada.ca/neuro/sphere/v1/audio/apps/products/programmes-v2/cestsibon/{}?context=web&pageNumber={}";

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
}
