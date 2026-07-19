#[cfg(not(feature = "throttling"))]
mod rxcursor;
#[cfg(feature = "throttling")]
mod rxcursor2;
use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result};
use rodio::cpal::traits::HostTrait;
use rodio::{Decoder, DeviceSinkBuilder, DeviceTrait, cpal};
pub use rodio::{MixerDeviceSink, Player};
use rxcursor::RxCursor;

pub fn start(url: &str) -> Result<MixerDeviceSink> {
    let rx = hls_handler::start(url)?;

    let mut cfg = std::env::current_exe()?;
    cfg.set_extension("cfg");

    let builder = if cfg.is_file() {
        let mut cfg_file = File::open(cfg)?;
        let mut device_name = String::new();
        cfg_file.read_to_string(&mut device_name)?;

        let mut devices = cpal::default_host().output_devices()?;
        match devices.find(|device| device.description().unwrap().name() == device_name) {
            Some(device) => {
                println!("Output device: {device_name}");
                DeviceSinkBuilder::from_device(device)?
            }
            None => DeviceSinkBuilder::from_default_device()?,
        }
    } else {
        DeviceSinkBuilder::from_default_device()?
    };

    let sink = builder.open_sink_or_fallback()?;
    let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
    sink.mixer().add(source);

    Ok(sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn ohdio() {
        let player = match start("Fournir un url master.m3u8 validé") {
            Ok(sink) => rodio::Player::connect_new(sink.mixer()),
            Err(e) => {
                println!("{e:?}");
                return assert!(false);
            }
        };

        thread::sleep(Duration::from_secs(15));
        player.pause();
        thread::sleep(Duration::from_secs(3));
        player.play();
        thread::sleep(Duration::from_secs(3));
        player.set_volume(5.0);
        assert_eq!(player.volume(), 5.0);
        thread::sleep(Duration::from_secs(3));
        player.stop();
        // assert!(false); // pour visualiser le stdout
    }
}
