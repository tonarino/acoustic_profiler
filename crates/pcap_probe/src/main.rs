#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use std::time::Duration;

use clap::{command, Parser};
use composer_api::{Client, Event, EventKind, Packet};
use eyre::{eyre, Context, Result};
use pcap::Capture;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
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

    let device = pcap::Device::lookup()
        .context("cal list devices")?
        .ok_or(eyre!("no default device found"))?;
    println!("using device: {device:?}");

    let mut capture = Capture::from_device(device)
        .unwrap()
        .immediate_mode(true)
        .open()
        .context("can open the device for capture")?;

    // TODO: consider batching
    while let Ok(cap) = capture.next_packet() {
        let ts = Duration::new(
            cap.header.ts.tv_sec.unsigned_abs(),
            // One microsecond is 1000 nanoseconds.
            cap.header.ts.tv_usec.unsigned_abs() * 1000,
        );

        // TODO: use size of the packet to vary the event "strength"
        let event = Event::with_timestamp(EventKind::TestTick, ts);

        if let Err(err) = client.send(&Packet::from_event(event)) {
            eprintln!("Could not send packet {:?}", err)
        };
    }

    Ok(())
}
