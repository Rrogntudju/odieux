mod rxcursor;
use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, Sink};
use rxcursor::RxCursor;


pub fn start(url: &str) -> Result<(Sink, OutputStream)> {
    let rx = hls_handler::start(url)?;
    let (_output_stream, stream_handle) = OutputStream::try_default().context("Échec: création de OutputStream")?;
    let sink = Sink::try_new(&stream_handle).context("Échec: création de Sink")?;
    let source = Decoder::new(RxCursor::new(rx)?).context("Échec: création de Decoder")?;
    sink.append(source);

    Ok((sink, _output_stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    #[test]
    fn ohdio() {
        let (player, _output_stream) = match start("https://rcavmedias-vh.akamaihd.net/i/5a5d3f75-97a9-47cd-be1c-70938e97f3d8/secured/2021-08-08_16_00_00_cestsibon_0000_,64,128,.mp4.csmil/master.m3u8?hdnea=st=1633815757~exp=1633815877~acl=/i/5a5d3f75-97a9-47cd-be1c-70938e97f3d8/secured/2021-08-08_16_00_00_cestsibon_0000_,*~hmac=44c6fd5088c7ba3dcccf24590e5be77160f5ea09f9bd386bb3667ca44db6127b") {
            Ok((s, o)) => (s, o),
            Err(e) => {
                println!("{:?}", e);
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
