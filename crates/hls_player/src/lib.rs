#[cfg(not(feature = "throttling"))]
mod rxcursor;
#[cfg(feature = "throttling")]
mod rxcursor2;
use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result};
use rodio::cpal::traits::HostTrait;
use rodio::{Decoder, DeviceTrait, cpal};
pub use rodio::{MixerDeviceSink, DeviceSinkBuilder, Player};
use rxcursor::RxCursor;

pub fn start(url: &str) -> Result<(Player, MixerDeviceSink)> {
    let rx = hls_handler::start(url)?;

    let mut cfg = std::env::current_exe()?;
    cfg.set_extension("cfg");
    let mut sink = DeviceSinkBuilder::open_default_sink()?;
    if cfg.is_file() {
        let mut cfg_file = File::open(cfg)?;
        let mut device_name = String::new();
        cfg_file.read_to_string(&mut device_name)?;

        let devices = cpal::default_host().output_devices()?;
        for device in devices { 
            if device.description()?.name() == device_name {
                println!("Output device: {device_name}");
                sink = DeviceSinkBuilder::from_device(device)?.open_stream()?;
                break;
            }    
        }
    };

    let player = Player::connect_new(sink.mixer());
    let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
    player.append(source);

    Ok((player, sink))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn ohdio() {
        let (player, _output_stream) = match start("Fournir un url master.m3u8 validé") {
            Ok((s, o)) => (s, o),
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
