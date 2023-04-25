#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use eyre::Result;
use serde::{Deserialize, Serialize};
use std::net::{ToSocketAddrs, UdpSocket};

pub const DEFAULT_SERVER_ADDRESS: &str = "localhost:8888";

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    TestTick,
    StdoutWrite { length: usize },
    StderrWrite { length: usize },
}

pub struct Client {
    socket: UdpSocket,
}

impl Client {
    pub fn try_default() -> Result<Self> {
        Self::new(DEFAULT_SERVER_ADDRESS)
    }

    pub fn new(server_address: impl ToSocketAddrs) -> Result<Self> {
        let socket = UdpSocket::bind("localhost:0")?;
        socket.connect(server_address)?;

        Ok(Self { socket })
    }

    pub fn send(&self, event: &Event) -> Result<()> {
        let data = bincode::serialize(event)?;
        self.socket.send(&data)?;
        Ok(())
    }
}
