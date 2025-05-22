use media::{get_episodes, get_media_id};
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::env::args;
use std::error::Error;
use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};

const TIME_OUT: u64 = 10;
const URL_VALIDEUR_OD: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianet&connectionType=hd&deviceType=ipad&idMedia={}&multibitrate=true&output=json&tech=hls&manifestVersion=2";
const URL_VALIDEUR_LIVE: &str = "https://services.radio-canada.ca/media/validation/v2/?appCode=medianetlive&connectionType=hd&deviceType=ipad&idMedia=cbvx&multibitrate=true&output=json&tech=hls&manifestVersion=2";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (url, titre) = if args().len() > 1 {
        let erreur = "Args: <id du programme> <page> <no de l'episode>";
        let mut args = args();
        let prog = match args.nth(1) {
            Some(arg) => arg.parse::<usize>().unwrap_or_default(),
            None => return Err(erreur.into()),
        };

        let page = match args.next() {
            Some(arg) => arg.parse::<usize>().unwrap_or_default(),
            None => return Err(erreur.into()),
        };

        let episode_no = match args.next() {
            Some(arg) => arg,
            None => return Err(erreur.into()),
        };

        let episodes = get_episodes(prog, page).await?;
        let no = episode_no.parse::<usize>()?.clamp(1, episodes.len()) - 1;
        let media_id = get_media_id(&episodes[no].id).await?;

        (URL_VALIDEUR_OD.replace("{}", &media_id), episodes[no].titre.trim().to_owned())
    } else {
        (URL_VALIDEUR_LIVE.to_owned(), "direct".to_owned())
    };
    let client = Client::builder().timeout(Duration::from_secs(TIME_OUT)).build()?;
    let task = tokio::spawn(client.get(&url).send().await?.text());
    let mut aac = env::temp_dir();
    aac.set_file_name(&titre);
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
