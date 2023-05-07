#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::util::current_timestamp;
use crate::{
    audio_output::AudioOutput,
    jukebox::{Jukebox, Sample},
};
use clap::Parser;
use composer_api::{EventKind, EventMessage, DEFAULT_SERVER_ADDRESS};
use eyre::{Context, Result};
use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

mod audio_output;
mod jukebox;
mod util;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// the address to listen on for incoming events
    address: Option<String>,

    /// Delay event timestamps by this amount during playback. Should be larger than audio buffer
    /// period time plus the sound card latency.
    #[arg(short, long, default_value_t = 200)]
    delay_ms: u64,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let socket = UdpSocket::bind(args.address.as_deref().unwrap_or(DEFAULT_SERVER_ADDRESS))?;
    println!("Listening on {}", socket.local_addr()?);

    let audio_output = AudioOutput::new(Duration::from_millis(args.delay_ms))?;

    let jukebox = Jukebox::new().context("creating jukebox")?;
    let mut stats = Stats { since: Instant::now(), events: 0, total_bytes: 0 };
    loop {
        match handle_datagram(&socket, &audio_output, &jukebox) {
            Ok(bytes_received) => stats.record_event(bytes_received, &audio_output),
            Err(err) => eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err),
        }
    }
}

/// Block until next datagram is received and handle it. Returns its size in bytes.
fn handle_datagram(
    socket: &UdpSocket,
    audio_output: &AudioOutput,
    jukebox: &Jukebox,
) -> Result<usize> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let message: EventMessage = bincode::deserialize(&buf[..number_of_bytes])?;

    for event in message.events {
        let sample = match event.kind {
            EventKind::TestTick => Sample::Clack,

            // TODO(Matej): add different sounds for these, and vary some their quality based on length.
            EventKind::StderrWrite { length: _ }
            | EventKind::StdoutWrite { length: _ }
            | EventKind::FileSystemWrite
            | EventKind::FileSystemRead => Sample::Click,
            // TODO(Pablo): Play a sound that scales with the number of reports.
            EventKind::LogStats(_) => todo!(),
        };
        let timestamp = event.timestamp.unwrap_or_else(current_timestamp);
        jukebox.play(audio_output, sample, timestamp);
    }

    Ok(number_of_bytes)
}

struct Stats {
    since: Instant,
    events: usize,
    total_bytes: usize,
}

impl Stats {
    const REPORT_EVERY: Duration = Duration::from_secs(1);

    fn record_event(&mut self, bytes_received: usize, audio_output: &AudioOutput) {
        self.events += 1;
        self.total_bytes += bytes_received;

        let elapsed = self.since.elapsed();
        if elapsed >= Self::REPORT_EVERY {
            println!(
                "Received {} events ({} bytes) in last {elapsed:.2?}, {} too early plays.",
                self.events,
                self.total_bytes,
                audio_output.fetch_too_early_plays(),
            );

            self.since = Instant::now();
            self.events = 0;
            self.total_bytes = 0;
        }
    }
}
