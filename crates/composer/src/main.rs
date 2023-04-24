#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::jukebox::{Jukebox, Sample};
use composer_api::{Event, DEFAULT_SERVER_ADDRESS};
use eyre::{Context, Result};
use rodio::{OutputStream, OutputStreamHandle};
use std::net::UdpSocket;

mod jukebox;

fn main() -> Result<()> {
    color_eyre::install()?;

    let socket = UdpSocket::bind(DEFAULT_SERVER_ADDRESS)?;
    println!("Listening on {}", socket.local_addr()?);

    let (_stream, stream_handle) = OutputStream::try_default()?;

    let jukebox = Jukebox::new().context("loading records into jukebox")?;
    loop {
        if let Err(err) = handle_datagram(&socket, &stream_handle, &jukebox) {
            eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err);
        }
    }
}

fn handle_datagram(
    socket: &UdpSocket,
    output_stream: &OutputStreamHandle,
    jukebox: &Jukebox,
) -> Result<()> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let event: Event = bincode::deserialize(&buf[..number_of_bytes])?;
    println!("Received an event ({number_of_bytes} bytes): {:?}", event);

    let sample = match event {
        Event::TestTick => Sample::Click,
    };
    jukebox.play(output_stream, sample)?;

    Ok(())
}
