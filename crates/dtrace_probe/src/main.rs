use clap::Parser;
use composer_api::{Client, Event, EventKind, Packet};
use dtrace::{DTrace, ProgramStatus};
use eyre::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[structopt(short, long)]
    server_address: Option<String>,

    #[structopt(short, long)]
    process_id: Option<u32>,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let running = Arc::new(AtomicBool::new(true));
    ctrlc::set_handler({
        let running = running.clone();
        move || {
            running.store(false, Ordering::SeqCst);
        }
    })
    .expect("Failed to set Ctrl-C handler");

    let client = match args.server_address {
        Some(address) => Client::new(address)?,
        None => Client::try_default()?,
    };

    let mut dtrace = DTrace::new()?;

    // We need to set the bufsize option explicitly; otherwise we get "Enabling exceeds size of
    // buffer" error.
    let options = &[("bufsize", "4m")];
    let predicate =
        args.process_id.map(|pid| format!("/pid == {pid}/")).unwrap_or_else(|| "".to_string());
    dtrace.execute_program(
        &format!(
            "syscall:::entry {predicate} {{ trace(walltimestamp); trace(arg0); trace(arg1); }}"
        ),
        options,
    )?;

    while running.load(Ordering::SeqCst) {
        let mut packet = Packet::default();

        let result = dtrace.wait_and_consume()?;

        for probe in &result.probes {
            let timestamp = Duration::from_nanos(probe.traces[0].parse().expect("Failed to parse timestamp"));
            let arg0 = &probe.traces[1];

            let kind = match probe.function_name.as_str() {
                "read" | "read_nocancel" => EventKind::FileSystemRead,
                "write" | "write_nocancel" => match arg0.as_str() {
                    "1" => EventKind::StdoutWrite { length: 0 },
                    "2" => EventKind::StderrWrite { length: 0 },
                    _ => EventKind::FileSystemWrite,
                },
                _ => continue,
            };

            let event = Event::with_timestamp(kind, timestamp);
            packet.events.push(event);
        }
        if !packet.events.is_empty() {
            if let Err(err) = client.send(&packet) {
                eprintln!("Could not send event {:?}", err)
            };
        }
        if result.status == ProgramStatus::Done {
            break;
        }
    }

    dtrace.stop()?;

    Ok(())
}
