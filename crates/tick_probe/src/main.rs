use clap::Parser;
use composer_api::{Client, Event};
use eyre::Result;
use std::{thread, time::Duration};

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

    let event = Event::TestTick;

    loop {
        if let Err(err) = client.send(&event) {
            eprintln!("Could not send event {:?}", err)
        };
        thread::sleep(Duration::from_secs_f64(1.0 / args.frequency));
    }
}
