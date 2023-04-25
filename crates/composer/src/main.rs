#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::jukebox::{Jukebox, Sample};
use composer_api::{Event, DEFAULT_SERVER_ADDRESS};
use eyre::{Context, Result};
use rodio::{OutputStream, OutputStreamHandle};
use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

mod jukebox;

fn main() -> Result<()> {
    color_eyre::install()?;

    let socket = UdpSocket::bind(DEFAULT_SERVER_ADDRESS)?;
    println!("Listening on {}", socket.local_addr()?);

    let (_stream, stream_handle) = OutputStream::try_default()?;

    let jukebox = Jukebox::new().context("creating jukebox")?;
    let mut stats = Stats { since: Instant::now(), events: 0, total_bytes: 0 };
    loop {
        match handle_datagram(&socket, &stream_handle, &jukebox) {
            Ok(bytes_received) => stats.record_event(bytes_received),
            Err(err) => eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err),
        }
    }
}

/// Block until next datagram is received and handle it. Returns its size in bytes.
fn handle_datagram(
    socket: &UdpSocket,
    output_stream: &OutputStreamHandle,
    jukebox: &Jukebox,
) -> Result<usize> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let event: Event = bincode::deserialize(&buf[..number_of_bytes])?;

    let sample = match event {
        Event::TestTick => Sample::Click,

        // TODO(Matej): add different sounds for these, and vary some their quality based on length.
        Event::StderrWrite { length: _ } | Event::StdoutWrite { length: _ } => Sample::Click,
    };
    jukebox.play(output_stream, sample)?;

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
