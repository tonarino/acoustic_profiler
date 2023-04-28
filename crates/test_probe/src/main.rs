#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use clap::{Parser, Subcommand};
use composer_api::{Client, Event, EventKind, Packet};
use eyre::Result;
use std::{
    thread::sleep,
    time::{Duration, Instant},
};

#[derive(Clone, Subcommand, Debug)]
enum Mode {
    /// Send events regularly with given frequency.
    Constant {
        /// Frequency of the events to generate.
        frequency: f64,
    },
    /// Send events with frequency that fluctuates higher and lower.
    Rollercoaster,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[command(subcommand)]
    mode: Mode,

    /// Server address to receive events.
    #[arg(short, long)]
    address: Option<String>,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let client = match args.address {
        Some(address) => Client::new(address),
        None => Client::try_default(),
    }?;

    let event = Event::new(EventKind::TestTick);
    let packet = Packet::from_event(event);

    let send = || {
        if let Err(err) = client.send(&packet) {
            eprintln!("Could not send event {:?}", err)
        };
    };

    match args.mode {
        Mode::Constant { frequency } => constant_frequency(frequency, send),
        Mode::Rollercoaster => rollercoaster(send),
    }
}

fn constant_frequency(frequency: f64, send: impl Fn()) -> ! {
    // Prevent drifting away from the given frequency by computing the sleep duration for each cycle.
    let start = Instant::now();
    for deadline in (0..).map(|i| start + Duration::from_secs_f64(i as f64 / frequency)) {
        send();
        sleep(deadline.saturating_duration_since(Instant::now()));
    }

    unreachable!()
}

fn rollercoaster(send: impl Fn()) -> ! {
    // u64 taken by `from_millis` doesn't implement DoubleEndedIterator needed by `rev`
    // Use u32 explicitly and convert to u64.
    let slowdown = (5u32..200).step_by(5);
    let speedup = slowdown.clone().rev();

    for delay_ms in speedup.chain(slowdown).cycle() {
        send();
        sleep(Duration::from_millis(delay_ms.into()));
    }

    unreachable!()
}
