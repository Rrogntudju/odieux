use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;
use std::time::Duration;
use urlencoding::encode;

const TIME_OUT: u64 = 30;
const GRAPHQL: &str = "https://services.radio-canada.ca/bff/audio/graphql";
const POST: &str = include_str!("post.json");

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
        format!(r#"{{"persistedQuery":{{"version":1,"sha256Hash":"246ae53bd719ea2ac74d753b3cda2d54cbe9186ae4b12b0c76e9e5c18b275fcc"}}}}"#);

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
dbg!(&programme);
    let valeur: Value = serde_json::from_str(&programme)?;
    let items = valeur["data"]["program"]["episodes"]
        .as_array()
        .context("episodes n'est pas un array")?;

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

pub async fn get_media_id(épisode_id: &str) -> Result<String> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let post = POST.replace("{}", épisode_id);
    let data = match client.post(GRAPHQL).header("Content-Type", "application/json").body(post).send().await {
        Ok(response) => response.text().await?,
        Err(e) => {
            if e.status() == Some(StatusCode::NOT_FOUND) {
                bail!("L'épisode {épisode_id} n'existe pas");
            } else {
                bail!(e);
            }
        }
    };
    let valeur: Value = serde_json::from_str(&data)?;
    let media_id = valeur["data"]["playbackListByGlobalId"]["items"][0]["mediaPlaybackItem"]["mediaId"]
        .as_str()
        .unwrap_or_default();
    ensure!(!media_id.is_empty(), "le media_id est nul");
    Ok(media_id.to_owned())
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

    #[tokio::test]
    async fn media_id() {
        match get_media_id("1094362").await {
            Ok(media_id) => assert_eq!(media_id, "10515519"),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }
}
