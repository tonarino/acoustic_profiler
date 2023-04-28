use log::{error, warn};
use std::time::Duration;

fn main() {
    log::set_boxed_logger(Box::new(
        log_probe::LogProbe::new(None, Duration::from_millis(500)).unwrap(),
    ))
    .unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    loop {
        std::thread::sleep(Duration::from_millis(100));
        error!("Oops, something bad happened");
        warn!("Oops, something not so bad happened");
        std::thread::sleep(Duration::from_millis(100));
        warn!("Oops, something not so bad happened");
    }
}
