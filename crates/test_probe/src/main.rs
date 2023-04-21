use composer::api::{Client, Event};
use eyre::Result;
use std::{env, thread, time::Duration};

fn main() -> Result<()> {
    color_eyre::install()?;

    // Get server address, if given
    let args: Vec<String> = env::args().collect();
    let arg = args.get(1);
    let server_address: Option<&str> = arg.map(|x: &String| x.as_str());

    let client = Client::new(server_address)?;
    let event = Event::TestTick;
    let delays = {
        let min_delay = 5;
        let max_delay = 250;
        let step = 10;
        // Tick fast, then slow, and then fast again
        (min_delay..max_delay)
            .step_by(step)
            .chain((min_delay..max_delay).step_by(step).rev())
    };

    loop {
        for delay in delays.clone() {
            if let Err(err) = client.send(&event) {
                eprintln!("Could not send event {:?}", err)
            };
            thread::sleep(Duration::from_millis(delay.try_into().unwrap()));
        }
    }
}