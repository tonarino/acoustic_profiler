use composer_api::{self, Client, LogStats};
use log::{self, Level};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Arc,
    },
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LogProbeError {
    #[error("Network error: {0}")]
    NetworkError(String),
}

pub struct LogProbe {
    tx: Sender<AggregatorMessage>,
    shutdown: Arc<AtomicBool>,
}

impl Drop for LogProbe {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

enum AggregatorMessage {
    AddRecord(Level),
    Tick,
}

impl LogProbe {
    pub fn new(
        server_address: Option<String>,
        report_frequency: Duration,
    ) -> Result<Self, LogProbeError> {
        let shutdown = Arc::<AtomicBool>::default();
        let client = if let Some(address) = server_address {
            Client::new(address)
        } else {
            Client::try_default()
        }
        .map_err(|e| LogProbeError::NetworkError(format!("{e}")))?;

        let (tx, rx) = channel::<AggregatorMessage>();
        spawn_aggregator_thread(rx, client, shutdown.clone());
        spawn_tick_thread(tx.clone(), shutdown.clone(), report_frequency);

        Ok(Self { tx, shutdown })
    }
}

fn spawn_tick_thread(
    tx: Sender<AggregatorMessage>,
    shutdown: Arc<AtomicBool>,
    report_frequency: Duration,
) {
    std::thread::spawn(move || {
        let start = Instant::now();
        for deadline in (0..).map(|i| start + i * report_frequency) {
            if let Err(e) = tx.send(AggregatorMessage::Tick) {
                eprintln!("Failed to communicate with aggregator thread.")
            }

            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(deadline - Instant::now());
        }
    });
}

fn spawn_aggregator_thread(
    rx: std::sync::mpsc::Receiver<AggregatorMessage>,
    client: Client,
    shutdown: Arc<AtomicBool>,
) {
    let mut log_stats = LogStats::default();
    let mut report_start = Instant::now();

    std::thread::spawn(move || {
        for message in rx {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            match message {
                AggregatorMessage::AddRecord(record) => add_record(&mut log_stats, record),
                AggregatorMessage::Tick => {
                    log_stats.span = report_start.elapsed();
                    if let Err(err) = client.send(&composer_api::Event::LogStats(log_stats)) {
                        eprintln!("Could not send event {:?}", err)
                    };
                    log_stats = Default::default();
                    report_start = Instant::now();
                },
            }
        }
    });
}

fn add_record(log_stats: &mut LogStats, record_level: Level) {
    match record_level {
        Level::Error => log_stats.error_records += 1,
        Level::Warn => log_stats.warn_records += 1,
        Level::Info => log_stats.info_records += 1,
        Level::Debug => log_stats.debug_records += 1,
        Level::Trace => log_stats.trace_records += 1,
    }
}

// impl log::Log for LogProbe {
//     fn enabled(&self, metadata: &log::Metadata) -> bool {
//         metadata.level() <= Level::Info
//     }

//     fn log(&self, record: &log::Record) {
//         todo!()
//     }

//     fn flush(&self) {}
// }
