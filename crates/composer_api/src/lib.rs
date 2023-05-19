#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use std::{
    net::{
        SocketAddr::{V4, V6},
        ToSocketAddrs, UdpSocket,
    },
    time::Duration,
};

pub const DEFAULT_SERVER_ADDRESS: &str = "localhost:8888";

/// Composer expects `Packet` as the incoming probe data.
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Packet {
    /// List of events a probe collected during a specific time window. For a probe generating
    /// high-frequency events e.g. more than a hundred per second, it's recommended to buffer and
    /// pack multiple events into a `Packet` to avoid overflowing the socket and reduce
    /// packet overhead.
    pub events: Vec<Event>,
}

impl Packet {
    pub fn new(events: Vec<Event>) -> Self {
        Self { events }
    }

    pub fn from_event(event: Event) -> Self {
        let events = vec![event];
        Self { events }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    /// Type of the event.
    pub kind: EventKind,

    /// Optional timestamp of the event, as the duration since UNIX epoch.
    pub timestamp: Option<Duration>,
}

impl Event {
    pub fn new(kind: EventKind) -> Self {
        let timestamp = None;
        Self { kind, timestamp }
    }

    pub fn with_timestamp(kind: EventKind, timestamp: Duration) -> Self {
        let timestamp = Some(timestamp);
        Self { kind, timestamp }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EventKind {
    TestTick,
    /// A write() syscall invocation to stdout.
    StdoutWrite {
        length: usize,
    },
    /// A write() syscall invocation to stderr.
    StderrWrite {
        length: usize,
    },
    /// A read() syscall invocation.
    FileSystemRead,
    /// A write() syscall invocation to a file other than stdout/stderr.
    FileSystemWrite,
    /// A log() invocation at a specified severity level.
    Log {
        level: LogLevel,
    },
    /// Logging events for a specific duration.
    LogStats(LogStats),
}

// FIXME: Duplicates the `log` crate definitions, but it's likely
// still better than pulling the dependency.
#[derive(Serialize, Deserialize, Debug)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Logs are aggregated by type (better for very high frequency logging)
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct LogStats {
    // Duration covered by this report.
    pub span: Duration,

    // Number of records of each kind.
    pub error_records: u32,
    pub warn_records: u32,
    pub info_records: u32,
    pub debug_records: u32,
    pub trace_records: u32,
}

pub struct Client {
    socket: UdpSocket,
}

impl Client {
    pub fn try_default() -> Result<Self> {
        Self::new(DEFAULT_SERVER_ADDRESS)
    }

    pub fn new(server_address: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind(Self::get_local_address(&server_address)?)?;
        socket.connect(server_address)?;

        Ok(Self { socket })
    }

    pub fn send(&self, packet: &Packet) -> Result<()> {
        let data = bincode::serialize(packet)?;
        self.socket.send(&data)?;
        Ok(())
    }

    /// Given a server address, returns a wildcard address of the same family (IPv4 or 6)
    /// that can be used to bind a socket for connecting to the server.
    fn get_local_address(server_address: &impl ToSocketAddrs) -> Result<&'static str> {
        let server_address = server_address
            .to_socket_addrs()?
            .next()
            .ok_or(eyre!("can't resolve server address"))?;

        // Set the address and port to 0 to let the OS choose unoccupied values for us
        match server_address {
            V4(_) => Ok("0.0.0.0:0"),
            V6(_) => Ok(":::0"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn returns_ipv4_wildcard_for_ipv4_address() {
        assert_eq!(Client::get_local_address(&"8.8.8.8:8888").unwrap(), "0.0.0.0:0");
    }

    #[test]
    fn returns_ipv6_wildcard_for_ipv6_address() {
        assert_eq!(Client::get_local_address(&"8::8:8888").unwrap(), ":::0");
    }
}
