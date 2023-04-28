use clap::Parser;
use composer_api::{Client, Event};
use dtrace::{DTrace, ProgramStatus};
use eyre::Result;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[structopt(short, long)]
    server_address: Option<String>,

    #[structopt(short, long)]
    process_id: u32,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let client = match args.server_address {
        Some(address) => Client::new(address)?,
        None => Client::try_default()?,
    };

    let mut dtrace = DTrace::new()?;

    // We need to set the bufsize option explicitly; otherwise we get "Enabling exceeds size of
    // buffer" error.
    let options = &[("bufsize", "4m")];
    let pid = args.process_id;
    dtrace.execute_program(
        &format!(
            "syscall:::entry /pid == {pid}/ {{ trace(walltimestamp); trace(arg0); trace(arg1); }}"
        ),
        options,
    )?;

    loop {
        let result = dtrace.wait_and_consume()?;
        for probe in &result.probes {
            // TODO(skywhale): Use this timestamp.
            let _timestamp: u128 = probe.traces[0].parse()?;
            let arg0 = &probe.traces[1];

            let event = match probe.function_name.as_str() {
                "read" | "read_nocancel" => Event::FileSystemRead,
                "write" | "write_nocancel" => match arg0.as_str() {
                    "1" => Event::StdoutWrite { length: 0 },
                    "2" => Event::StderrWrite { length: 0 },
                    _ => Event::FileSystemWrite,
                },
                _ => continue,
            };

            if let Err(err) = client.send(&event) {
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
