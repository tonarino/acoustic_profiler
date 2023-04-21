use eyre::Result;
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;

pub const DEFAULT_SERVER_ADDRESS: &str = "localhost:8888";

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    TestTick,
}

pub struct Client {
    socket: UdpSocket,
}

impl Client {
    pub fn new(server_address: Option<&str>) -> Result<Self> {
        let socket = UdpSocket::bind("localhost:0")?;
        socket.connect(server_address.unwrap_or(DEFAULT_SERVER_ADDRESS))?;

        Ok(Self { socket })
    }

    pub fn send(&self, event: &Event) -> Result<()> {
        let data = bincode::serialize(event)?;
        self.socket.send(&data)?;
        Ok(())
    }
}
