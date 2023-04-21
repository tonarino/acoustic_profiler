use composer::api::Event;
use composer::api::DEFAULT_SERVER_ADDRESS;
use eyre::Result;
use rodio::{source::Source, Decoder, OutputStream};
use std::fs::File;
use std::io::BufReader;
use std::net::UdpSocket;

fn main() -> Result<()> {
    color_eyre::install()?;

    let socket = UdpSocket::bind(DEFAULT_SERVER_ADDRESS)?;
    println!("Listening on {}", socket.local_addr()?);

    let (_stream, stream_handle) = OutputStream::try_default()?;

    loop {
        if let Err(err) = {
            // Size up to max normal network packet size should be enough
            let mut buf = [0; 1500];
            let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

            let event: Event = bincode::deserialize(&buf)?;
            println!("Received an event ({number_of_bytes} bytes): {:?}", event);

            // FIXME: do the decoding and file reading outside the loop
            let file = BufReader::new(File::open("src/sound_samples/click.wav")?);
            let source = Decoder::new(file)?;
            stream_handle.play_raw(source.convert_samples())?;

            Ok::<(), eyre::Error>(())
        } {
            eprintln!("Could not process data-gram: {:?}. Ignoring and continuing.", err);
            continue;
        }
    }
}
