use std::error::Error;
use std::default::Default;
use serde::Serialize;
use std::env;
use std::io::BufWriter;
use std::fs::File;

#[derive(Serialize)]
struct Emission {
    titre: String,
    url: String,
}

#[derive(Serialize, Default)]
struct Emissions(Vec<Emission>);

fn gratte(url: &str, out: &str) ->  Result<(), Box<dyn Error>> {
    let émissions = Emissions::default();
    let mut json = env::temp_dir();
    json.push(out);
    let mut file = BufWriter::new(File::create(json)?);
    for i in 1.. {

    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    gratte("https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber={}", "csb.json")?;
    Ok(())
}
