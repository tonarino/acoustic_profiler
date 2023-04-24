use composer::api::{Client, Event};
use dtrace::{DTrace, ProgramStatus};
use eyre::Result;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "DTrace Probe", about = "Options for DTrace probe")]
struct Opt {
    #[structopt(short, long)]
    server_address: Option<String>,

    #[structopt(short, long)]
    process_id: u32,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let opt = Opt::from_args();

    let client = match opt.server_address {
        Some(address) => Client::new(address)?,
        None => Client::try_default()?,
    };

    let mut dtrace = DTrace::new()?;

    let pid = opt.process_id;
    let options = [("bufsize", "1k")];
    dtrace.execute_program(&format!("syscall:::entry /pid == {pid}/ {{}}"), &options)?;

    let mut probes = Vec::default();
    while dtrace.wait_and_consume(&mut probes)? != ProgramStatus::Done {
        for probe in &probes {
            let event = match probe.function_name.as_str() {
                "read" | "read_nocancel" => Event::FileSystemRead,
                "write" | "write_nocancel" => Event::FileSystemWrite,
                func => {
                    println!("syscall {}", func);
                    continue;
                },
            };
            if let Err(err) = client.send(&event) {
                eprintln!("Could not send event {:?}", err)
            };
        }
    }

    dtrace.stop()?;

    Ok(())
}
