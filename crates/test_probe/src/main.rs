#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use clap::Parser;
use composer_api::{Client, Event, EventKind, EventMessage};
use eyre::Result;
use std::{thread, time::Duration};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
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
    let message = EventMessage::with_event(event);

    // u64 taken by `from_millis` doesn't implement DoubleEndedIterator needed by `rev`
    // Use u32 explicitly and convert to u64.
    let slowdown = (5u32..200).step_by(5);
    let speedup = slowdown.clone().rev();

    for delay_ms in speedup.chain(slowdown).cycle() {
        if let Err(err) = client.send(&message) {
            eprintln!("Could not send event {:?}", err)
        };
        thread::sleep(Duration::from_millis(delay_ms.into()));
    }

    // TODO: Result<!> isn't supported yet. Change the return type and remove
    // this once it is. https://github.com/rust-lang/rust/issues/35121
    unreachable!()
}
