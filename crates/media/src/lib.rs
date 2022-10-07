use anyhow::{ensure, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;

#[derive(Deserialize, Serialize, Default, Clone, PartialEq)]
pub struct Episode {
    pub titre: String,
    pub media_id: String,
}

pub async fn get_episodes(url: &str, client: &Client) -> Result<Vec<Episode>> {
    let page = client.get(url).send().await?.text().await?;
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
    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn csb() {
        let client = Client::builder().timeout(Duration::from_secs(10)).build().unwrap();
        match get_episodes(
            "https://services.radio-canada.ca/neuro/sphere/v1/audio/apps/products/programmes-v2/cestsibon/1?context=web&pageNumber=1",
            &client,
        )
        .await
        {
            Ok(_) => assert!(true),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }
}
