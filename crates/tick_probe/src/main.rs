use clap::Parser;
use composer_api::{Client, Event, EventKind, Packet};
use eyre::Result;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Frequency of the tick events to generate.
    frequency: f64,

    /// Server address to receive events.
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

    // Try to prevent drifting away from the given frequency by dynamically computing the sleep duration for each cycle
    let start = Instant::now();
    for deadline in (0..).map(|i| start + Duration::from_secs_f64(i as f64 / args.frequency)) {
        if let Err(err) = client.send(&packet) {
            eprintln!("Could not send event {:?}", err)
        };

        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
    }

    unreachable!()
}
