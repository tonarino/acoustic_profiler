use composer_api::{self, Client, Event, EventKind, LogStats, Packet};
use log::{self, Level};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
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
    tx: SyncSender<AggregatorMessage>,
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

pub enum Mode {
    Aggregated,
    Individual,
}

impl LogProbe {
    pub fn new(
        server_address: Option<String>,
        report_period: Duration,
        mode: Mode,
    ) -> Result<Self, LogProbeError> {
        let shutdown = Arc::<AtomicBool>::default();
        let client = if let Some(address) = server_address {
            Client::new(address)
        } else {
            Client::try_default()
        }
        .map_err(|e| LogProbeError::NetworkError(format!("{e}")))?;

        let (tx, rx) = sync_channel::<AggregatorMessage>(100);
        match mode {
            Mode::Aggregated => spawn_aggregated_mode_thread(rx, client, shutdown.clone()),
            Mode::Individual => spawn_individual_mode_thread(rx, client, shutdown.clone()),
        }
        spawn_tick_thread(tx.clone(), shutdown.clone(), report_period);

        Ok(Self { tx, shutdown })
    }
}

fn spawn_tick_thread(
    tx: SyncSender<AggregatorMessage>,
    shutdown: Arc<AtomicBool>,
    report_period: Duration,
) {
    std::thread::spawn(move || {
        let start = Instant::now();
        for deadline in (1..).map(|i| start + i * report_period) {
            if let Err(e) = tx.try_send(AggregatorMessage::Tick) {
                eprintln!("Failed to communicate with aggregator thread: {e}")
            }

            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(deadline - Instant::now());
        }
    });
}

fn spawn_aggregated_mode_thread(
    rx: Receiver<AggregatorMessage>,
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
                AggregatorMessage::AddRecord(record) => {
                    add_aggregate_record(&mut log_stats, record)
                },
                AggregatorMessage::Tick => {
                    log_stats.span = report_start.elapsed();
                    let event = Event::new(EventKind::LogStats(log_stats));
                    if let Err(err) = client.send(&Packet::from_event(event)) {
                        eprintln!("Could not send event {:?}", err)
                    }
                    log_stats = Default::default();
                    report_start = Instant::now();
                },
            }
        }
    });
}

fn spawn_individual_mode_thread(
    rx: Receiver<AggregatorMessage>,
    client: Client,
    shutdown: Arc<AtomicBool>,
) {
    let mut events = Vec::new();

    std::thread::spawn(move || {
        for message in rx {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            match message {
                AggregatorMessage::AddRecord(record) => {
                    let level = match record {
                        Level::Error => composer_api::LogLevel::Error,
                        Level::Warn => composer_api::LogLevel::Warn,
                        Level::Info => composer_api::LogLevel::Info,
                        Level::Debug => composer_api::LogLevel::Debug,
                        Level::Trace => composer_api::LogLevel::Trace,
                    };
                    let event = Event::with_current_timestamp(EventKind::Log { level });
                    events.push(event);
                },
                AggregatorMessage::Tick => {
                    if let Err(err) = client.send(&Packet::new(events)) {
                        eprintln!("Could not send event {:?}", err)
                    };
                    events = Vec::new();
                },
            }
        }
    });
}

fn add_aggregate_record(log_stats: &mut LogStats, record_level: Level) {
    match record_level {
        Level::Error => log_stats.error_records += 1,
        Level::Warn => log_stats.warn_records += 1,
        Level::Info => log_stats.info_records += 1,
        Level::Debug => log_stats.debug_records += 1,
        Level::Trace => log_stats.trace_records += 1,
    }
}

impl log::Log for LogProbe {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if let Err(e) = self.tx.send(AggregatorMessage::AddRecord(record.level())) {
            eprintln!("Failed to communicate with aggregator thread: {e}")
        };
    }

    fn flush(&self) {}
}
