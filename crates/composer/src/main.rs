#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use crate::{
    jukebox::{Jukebox, Sample},
    sound::AudioOutput,
};
use clap::Parser;
use composer_api::{Event, DEFAULT_SERVER_ADDRESS};
use eyre::{Context, Result};
use std::{
    collections::BTreeMap,
    net::UdpSocket,
    time::{Duration, Instant},
};

mod jukebox;
mod sound;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// the address to listen on for incoming events
    address: Option<String>,
}

type EventQueue = BTreeMap<Instant, Event>;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let socket = UdpSocket::bind(args.address.as_deref().unwrap_or(DEFAULT_SERVER_ADDRESS))?;
    println!("Listening on {}", socket.local_addr()?);

    let audio_output = AudioOutput::new()?;

    let jukebox = Jukebox::new().context("creating jukebox")?;
    let mut stats = Stats { since: Instant::now(), events: 0, total_bytes: 0 };
    let mut event_queue: EventQueue = BTreeMap::default();

    loop {
        match handle_datagram(&socket, &mut event_queue) {
            Ok(bytes_received) => stats.record_event(bytes_received),
            Err(err) => eprintln!("Could not process datagram. Ignoring and continuing. {:?}", err),
        }

        tick_synthesis(&mut event_queue, &audio_output, &jukebox)
        // TODO: play sound tick
    }
}

/// Block until next datagram is received and handle it. Returns its size in bytes.
fn handle_datagram(socket: &UdpSocket, event_queue: &mut EventQueue) -> Result<usize> {
    // Size up to max normal network packet size
    let mut buf = [0; 1500];
    let (number_of_bytes, _) = socket.recv_from(&mut buf)?;

    let event: Event = bincode::deserialize(&buf[..number_of_bytes])?;

    event_queue.insert(Instant::now(), event);

    Ok(number_of_bytes)
}

fn tick_synthesis(event_queue: &mut EventQueue, audio_output: &AudioOutput, jukebox: &Jukebox) {
    let ts = match event_queue.last_key_value() {
        Some((ts, _)) => ts,
        None => return,
    };

    if *ts > Instant::now() {
        return;
    }

    let (_, event) = event_queue.pop_last().expect("entry disappeared under our hands");

    let sample = match event {
        Event::TestTick => Sample::Clack,
        Event::StdoutWrite { length: _ } | Event::StderrWrite { length: _ } => Sample::Click,
    };

    jukebox.play(audio_output, sample);
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
