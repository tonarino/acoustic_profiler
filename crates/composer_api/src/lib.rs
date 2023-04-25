#![warn(clippy::all, clippy::clone_on_ref_ptr)]

use eyre::{eyre, Result};
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
        let socket = UdpSocket::bind(Self::get_local_address(&server_address)?)?;
        socket.connect(server_address)?;

        Ok(Self { socket })
    }

    pub fn send(&self, event: &Event) -> Result<()> {
        let data = bincode::serialize(event)?;
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
        if server_address.is_ipv4() {
            Ok("0.0.0.0:0")
        } else if server_address.is_ipv6() {
            Ok(":::0")
        } else {
            Err(eyre!("not an IP address"))
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
