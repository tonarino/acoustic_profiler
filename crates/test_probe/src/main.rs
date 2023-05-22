#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use clap::{Parser, Subcommand};
use composer_api::{util::current_timestamp, Client, Event, EventKind, Packet};
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
    /// Create timestamped events evenly spaced in time but send them in bursts.
    Burst {
        #[arg(short, long, default_value_t = 500)]
        burst_period_ms: u64,
        #[arg(short, long, default_value_t = 50)]
        events_per_burst: u32,
    },
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

    let send = |packet: &Packet| {
        if let Err(err) = client.send(packet) {
            eprintln!("Could not send packet {:?}", err)
        };
    };

    match args.mode {
        Mode::Constant { frequency } => constant_frequency(frequency, send),
        Mode::Rollercoaster => rollercoaster(send),
        Mode::Burst { burst_period_ms, events_per_burst } => {
            burst(Duration::from_millis(burst_period_ms), events_per_burst, send)
        },
    }
}

fn constant_frequency(frequency: f64, send: impl Fn(&Packet)) -> ! {
    let event = Event::new(EventKind::TestTick);
    let packet = Packet::from_event(event);
    // Prevent drifting away from the given frequency by computing the sleep duration for each cycle.
    let start = Instant::now();
    for deadline in (1..).map(|i| start + Duration::from_secs_f64(i as f64 / frequency)) {
        send(&packet);
        sleep(deadline.saturating_duration_since(Instant::now()));
    }

    unreachable!()
}

fn rollercoaster(send: impl Fn(&Packet)) -> ! {
    let event = Event::new(EventKind::TestTick);
    let packet = Packet::from_event(event);

    // u64 taken by `from_millis` doesn't implement DoubleEndedIterator needed by `rev`
    // Use u32 explicitly and convert to u64.
    let slowdown = (5u32..200).step_by(5);
    let speedup = slowdown.clone().rev();

    for delay_ms in speedup.chain(slowdown).cycle() {
        send(&packet);
        sleep(Duration::from_millis(delay_ms.into()));
    }

    unreachable!()
}

fn burst(burst_period: Duration, events_per_burst: u32, send: impl Fn(&Packet)) -> ! {
    let event_period = burst_period / events_per_burst;
    // Delay events' timestamps by one burst period to ensure they're not set in future when sent out.
    let event_start = current_timestamp() - burst_period;
    let mut event_generator =
        (0..).map(|n| Event::with_timestamp(EventKind::TestTick, event_start + n * event_period));

    // Prevent drifting away from the given frequency by computing the sleep duration for each cycle.
    let start = Instant::now();
    for deadline in (1..).map(|i| start + i * burst_period) {
        let events = event_generator.by_ref().take(events_per_burst as usize).collect();
        let packet = Packet::new(events);
        send(&packet);

        sleep(deadline.saturating_duration_since(Instant::now()));
    }

    unreachable!()
}
