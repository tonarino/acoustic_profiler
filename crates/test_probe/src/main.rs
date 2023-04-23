use composer::api::{Client, Event};
use eyre::Result;
use std::{env, thread, time::Duration};

fn main() -> Result<()> {
    color_eyre::install()?;

    // Get server address, if given
    let args: Vec<String> = env::args().collect();
    let client = match args.get(1) {
        Some(address) => Client::new(address)?,
        None => Client::try_default()?,
    };

    let event = Event::TestTick;

    // u64 taken by `from_millis` doesn't implement DoubleEndedIterator needed by `rev`
    // Use u32 explicitly and convert to u64.
    let slowdown = (5u32..200).step_by(5);
    let speedup = slowdown.clone().rev();

    for delay_ms in speedup.chain(slowdown).cycle() {
        if let Err(err) = client.send(&event) {
            eprintln!("Could not send event {:?}", err)
        };
        thread::sleep(Duration::from_millis(delay_ms.into()));
    }

    // TODO: Result<!> isn't supported yet. Change the return type and remove
    // this once it is. https://github.com/rust-lang/rust/issues/35121
    unreachable!()
}
