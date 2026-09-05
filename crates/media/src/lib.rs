use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;
use std::time::Duration;
use urlencoding::encode;

const TIME_OUT: u64 = 30;
const GRAPHQL: &str = "https://services.radio-canada.ca/bff/audio/graphql";

#[derive(Deserialize, Serialize, Default, Clone, PartialEq, Debug)]
pub struct Episode {
    pub titre: String,
    pub media_id: String,
}

// Chaque page du programme contient jusqu'à 50 épisodes (2026/07/14)
pub async fn get_episodes(prog_id: usize, page_no: usize) -> Result<Vec<Episode>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let opname = "programmeById";

    // Le format! est nécessaire pour que {{}} devienne {}
    let extensions =
        format!(r#"{{"persistedQuery":{{"version":1,"sha256Hash":"8aab6fa465968cefee6d31643c85170454dc79fe942ec56c0269fcde03fb83c3"}}}}"#);

    let variables = format!(r#"{{"params":{{"device":"Web","id":{prog_id},"pageNumber":{page_no}}}}}"#);
    let url = format!(
        "{}?opname={}&extensions={}&variables={}",
        GRAPHQL,
        opname,
        &encode(&extensions),
        &encode(&variables)
    );

    let programme = match client.get(&url).header("Content-Type", "application/json").send().await {
        Ok(response) => response.text().await?,
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                bail!("Le programme {prog_id} ou la page {page_no} n'existe pas");
            } else {
                bail!(e);
            }
        }
    };

    let valeur: Value = serde_json::from_str(&programme)?;
    let items = valeur["data"]["program"]["episodes"].as_array().context("episodes n'est pas un array")?;

    let mut épisodes = Vec::new();
    for item in items {
        ensure!(item.is_object(), "item n'est pas un objet");

        let titre = item["appShare"]["title"].as_str().unwrap_or_default();
        ensure!(!titre.is_empty(), "le titre est nul");

        let media_id = item["mediaIds"][0].as_u64().unwrap_or(0);
        ensure!(!media_id != 0, "le media_id est nul");

        épisodes.push(Episode {
            titre: titre.to_owned(),
            media_id: media_id.to_string(),
        });
    }

    Ok(épisodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn épisodes() {
        match get_episodes(5325, 1).await {
            Ok(_) => assert!(true),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }
}
