use composer_api::Event;
use composer_api::DEFAULT_SERVER_ADDRESS;
use eyre::Result;
use rodio::{source::Source, Decoder, OutputStream, OutputStreamHandle};
use std::fs::File;
use std::io::BufReader;
use std::net::UdpSocket;

fn main() -> Result<()> {
    color_eyre::install()?;

    let socket = UdpSocket::bind(DEFAULT_SERVER_ADDRESS)?;
    println!("Listening on {}", socket.local_addr()?);

    let (_stream, stream_handle) = OutputStream::try_default()?;

    loop {
        if let Err(err) = handle_datagram(&socket, &stream_handle) {
            eprintln!(
                "Could not process datagram. Ignoring and continuing. {:?}",
                err
            );
        }
    }
}

fn handle_datagram(socket: &UdpSocket, output_stream: &OutputStreamHandle) -> Result<()> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let event: Event = bincode::deserialize(&buf[..number_of_bytes])?;
    println!("Received an event ({number_of_bytes} bytes): {:?}", event);

    // FIXME: do the decoding and file reading outside the loop
    let file = BufReader::new(File::open("src/sound_samples/click.wav")?);
    let source = Decoder::new(file)?;
    output_stream.play_raw(source.convert_samples())?;

    Ok(())
}
