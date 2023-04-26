#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::sound::SoundController;
use clap::Parser;
use composer_api::{Event, DEFAULT_SERVER_ADDRESS};
use eyre::Result;
use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

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

    let mut stats = Stats { since: Instant::now(), events: 0, total_bytes: 0 };
    loop {
        match handle_datagram(&socket, &mut sound_controller) {
            Ok(bytes_received) => stats.record_event(bytes_received),
            Err(err) => eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err),
        }
    }
}

fn handle_datagram(socket: &UdpSocket, sound_controller: &mut SoundController) -> Result<usize> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let _event: Event = bincode::deserialize(&buf[..number_of_bytes])?;

    sound_controller.play_click();

    Ok(number_of_bytes)
}

struct Stats {
    since: Instant,
    events: usize,
    total_bytes: usize,
}

impl Stats {
    const REPORT_EVERY: Duration = Duration::from_secs(1);

    fn record_event(&mut self, bytes_received: usize) {
        self.events += 1;
        self.total_bytes += bytes_received;

        let elapsed = self.since.elapsed();
        if elapsed >= Self::REPORT_EVERY {
            println!(
                "Received {} events ({} bytes) in last {elapsed:.2?}.",
                self.events, self.total_bytes
            );

            self.since = Instant::now();
            self.events = 0;
            self.total_bytes = 0;
        }
    }
}
