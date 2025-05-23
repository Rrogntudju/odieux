use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::default::Default;
use std::time::Duration;
use urlencoding::encode;

const TIME_OUT: u64 = 30;
const GRAPHQL: &str = "https://services.radio-canada.ca/bff/audio/graphql";
const POST: &str = include_str!("post.txt");

#[derive(Deserialize, Serialize, Default, Clone, PartialEq, Debug)]
pub struct Episode {
    pub titre: String,
    pub id: String,
}

pub async fn get_episodes(prog_id: usize, page_no: usize) -> Result<Vec<Episode>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let opname = "programmeById";
    
    // Le format! est nécessaire pour que {{}} devienne {}
    let extensions =
        format!(r#"{{"persistedQuery":{{"version":1,"sha256Hash":"2d92832867f9f3b685fff3e5f213c3ff3414d02c74ee461580842cb6e31dedfd"}}}}"#);

    let variables = format!(r#"{{"params":{{"context":"web","forceWithoutCueSheet":false,"id":{prog_id},"pageNumber":{page_no}}}}}"#);
    let url = format!(
        "{}?opname={}&extensions={}&variables={}",
        GRAPHQL,
        opname,
        &encode(&extensions),
        &encode(&variables)
    );
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
    let items = valeur["data"]["programmeById"]["content"]["contentDetail"]["items"]
        .as_array()
        .context("items n'est pas un array")?;
    let mut épisodes = Vec::new();
    for item in items {
        ensure!(item.is_object(), "item n'est pas un objet");
        
        let titre = item["title"].as_str().unwrap_or_default();
        ensure!(!titre.is_empty(), "le titre est nul");

        let id = item["playlistItemId"]["globalId2"]["id"].as_str().unwrap_or_default();
        ensure!(!id.is_empty(), "l'id est nul");

        épisodes.push(Episode {
            titre: titre.to_owned(),
            id: id.to_owned(),
        });
    }
    ensure!(!épisodes.is_empty(), "La page {page_no} n'existe pas");
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
        match get_episodes(1161, 13).await {
            Ok(_) => assert!(true),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }

    #[tokio::test]
    async fn media_id() {
        match get_media_id("963208").await {
            Ok(media_id) => assert_eq!(media_id, "10362937"),
            Err(e) => {
                println!("{e:?}");
                assert!(false);
            }
        }
    }
}
