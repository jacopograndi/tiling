// net for native, but no net for wasm. fuck you wasm.
// crossplay with wasm would need (if it's even possible):
// - html3 connection implementation,
// - public relay server (a la matchbox),
// - webtransport implementation

use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{io, net::UdpSocket};

pub struct NetServer {
    socket: Option<UdpSocket>,
    pub client_connections: Vec<NetRemoteConnection>,
    pub port: u16,
}

pub struct NetRemoteConnection {
    pub address: SocketAddr,
    pub last_message_time: f64,
}

pub struct NetClient {
    socket: Option<UdpSocket>,
    pub server_address: Option<SocketAddr>,
}

impl NetServer {
    pub fn new() -> Self {
        Self {
            socket: None,
            client_connections: vec![],
            port: 0,
        }
    }

    pub fn open(&mut self, port: u16) -> io::Result<()> {
        self.socket = Some({
            let socket = UdpSocket::bind((IpAddr::V4(Ipv4Addr::UNSPECIFIED), port))?;
            socket.set_nonblocking(true)?;
            socket
        });
        self.port = port;
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.socket.is_some()
    }

    pub fn receive(&mut self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        if let Some(ref socket) = self.socket {
            let (amt, address) = socket
                .recv_from(buf)
                .map_err(|e| NetError::SocketError(e))?;

            if let Some(connection) = self
                .client_connections
                .iter_mut()
                .find(|conn| conn.address == address)
            {
                connection.last_message_time = miniquad::date::now();
            } else {
                self.client_connections.push(NetRemoteConnection {
                    address,
                    last_message_time: miniquad::date::now(),
                })
            }

            return Ok((amt, address));
        }
        Err(NetError::NoSocket)
    }

    pub fn send(&self, buf: &[u8], address: SocketAddr) -> Result<usize, NetError> {
        if let Some(ref socket) = self.socket {
            let amt = socket
                .send_to(buf, address)
                .map_err(|e| NetError::SocketError(e))?;
            return Ok(amt);
        }
        Err(NetError::NoSocket)
    }

    pub fn close(&mut self) {
        self.client_connections.clear();
        self.socket = None;
    }
}

impl NetClient {
    pub fn new() -> Self {
        Self {
            socket: None,
            server_address: None,
        }
    }

    pub fn connect(&mut self, server_address: SocketAddr) -> io::Result<()> {
        self.socket = Some({
            let socket = UdpSocket::bind("0.0.0.0:0")?;
            socket.set_nonblocking(true)?;
            socket
        });
        self.server_address = Some(server_address);
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.socket.is_some()
    }

    pub fn receive(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        if let Some(ref socket) = self.socket {
            let (amt, src) = socket
                .recv_from(buf)
                .map_err(|e| NetError::SocketError(e))?;
            return Ok((amt, src));
        }
        Err(NetError::NoSocket)
    }

    pub fn send(&self, buf: &[u8]) -> Result<usize, NetError> {
        if let (Some(ref socket), Some(ref address)) = (&self.socket, &self.server_address) {
            let amt = socket
                .send_to(buf, address)
                .map_err(|e| NetError::SocketError(e))?;
            return Ok(amt);
        }
        Err(NetError::NoSocket)
    }

    pub fn close(&mut self) {
        self.socket = None;
    }
}

#[derive(Debug)]
pub enum NetError {
    NoSocket,
    SocketError(io::Error),
}
