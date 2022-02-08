use gratte::gratte;
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::env::args;
use std::error::Error;
use std::io;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

const TIME_OUT: u64 = 10;
const CSB: &str = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber=";
const URL_VALIDEUR_OD: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls";
const URL_VALIDEUR_LIVE: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls";
const PAGES: usize = 68;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let (url, titre) = if args().len() > 1 {
        let page = match args().nth(1) {
            Some(arg) => arg,
            None => return Err("Fournir le numéro de la page".into()),
        };

        let num = match args().nth(2) {
            Some(arg) => arg,
            None => return Err("Fournir le numéro de l'épisode".into()),
        };

        let page = page.parse::<usize>()?.clamp(1, PAGES);
        let épisodes = gratte(CSB, page, &client).await?;

        let num = num.parse::<usize>()?.clamp(1, épisodes.len());
        let media_id = &épisodes[num - 1].media_id;
        if media_id.is_empty() {
            return Err("Aucune musique diffusée disponible".into());
        }
        (URL_VALIDEUR_OD.replace("{}", media_id), épisodes[num - 1].titre.trim().to_owned())
    } else {
        (URL_VALIDEUR_LIVE.to_owned(), "direct".to_owned())
    };
    let task = tokio::spawn(client.get(&url).send().await?.text());
    let mut aac = env::temp_dir();
    aac.push(&titre);
    aac.set_extension("aac");
    let mut file = BufWriter::new(File::create(aac).await?);
    let value: Value = serde_json::from_str(&task.await??)?;
    let rx = hls_handler::start(value["url"].as_str().unwrap_or_default())?;

    let signal = Arc::new(AtomicBool::new(false));
    let signal2 = signal.clone();
    tokio::task::spawn_blocking(move || {
        println!("Faites <Enter> pour interrompre...");
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
        signal2.store(true, Ordering::Relaxed);
    });

    for message in rx {
        match message {
            Ok(stream) => file.write_all(&stream).await?,
            Err(e) => return Err(e.into()),
        };
        if signal.load(Ordering::Relaxed) {
            break;
        }
    }
    file.flush().await?;

    Ok(())
}
