#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::sound::SoundController;
use clap::Parser;
use composer_api::{Event, DEFAULT_SERVER_ADDRESS};
use eyre::Result;
use std::net::UdpSocket;

mod jukebox;
mod sound;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// the address to listen on for incoming events
    address: Option<String>,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let socket = UdpSocket::bind(args.address.as_deref().unwrap_or(DEFAULT_SERVER_ADDRESS))?;
    println!("Listening on {}", socket.local_addr()?);

    let mut sound_controller = SoundController::new()?;

    loop {
        if let Err(err) = handle_datagram(&socket, &mut sound_controller) {
            eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err);
        }
    }
}

fn handle_datagram(socket: &UdpSocket, sound_controller: &mut SoundController) -> Result<()> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let event: Event = bincode::deserialize(&buf[..number_of_bytes])?;
    println!("Received an event ({number_of_bytes} bytes): {:?}", event);

    sound_controller.increment_hz();

    Ok(())
}
