fn main() {

use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink};
use rodio::source::{SineWave, Source};

let (_, stream_handle) = OutputStream::try_default().unwrap();
let sink = Sink::try_new(&stream_handle).unwrap();

// Add a dummy source of the sake of the example.
let source1 = SineWave::new(440).take_duration(Duration::from_secs_f32(1.0)).amplify(0.20);
let source2 = SineWave::new(440).take_duration(Duration::from_secs_f32(0.50)).amplify(0.20);
let file = BufReader::new(File::open(r"C:\Users\Scotty\Downloads\sample3.aac").unwrap());
// Decode that sound file into a source
let source3 = Decoder::new(file).unwrap();
sink.append(source1);
std::thread::sleep(Duration::from_secs_f32(0.50));
sink.append(source2);
std::thread::sleep(Duration::from_secs_f32(0.50));
sink.append(source3); 

// The sound plays in a separate thread. This call will block the current thread until the sink
// has finished playing all its queued sounds.
sink.sleep_until_end();
}
