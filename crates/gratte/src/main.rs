use std::error::Error;
use std::default::Default;
use serde::Serialize;

#[derive(Serialize, Default)]
struct Emissions {
    urls: Vec<String>,
}
    


fn main() -> Result<(), Box<dyn Error>> {
    
    let url = "https://ici.radio-canada.ca/ohdio/musique/emissions/1161/cestsibon?pageNumber={}";
    let émissions = Emissions::default();

    loop {

    }
    Ok(())
}
