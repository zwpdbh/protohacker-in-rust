use std::net::SocketAddr;
use tokio::net::UdpSocket;

use crate::{Error, Result};

pub struct LrcpListener {
    pub addr: SocketAddr,
    pub udp_socket: UdpSocket,
}

impl LrcpListener {
    pub async fn bind(addr: String) -> Result<Self> {
        let addr: SocketAddr = addr.parse().unwrap();
        let socket = UdpSocket::bind(addr).await?;

        Ok(LrcpListener {
            addr,
            udp_socket: socket,
        })
    }

    pub async fn accept(&self) -> Result<(LrcpStream, SocketAddr)> {
        let lrcp_stream = LrcpStream::new();
        Ok((lrcp_stream, self.addr))
    }
}

pub struct LrcpStream {}

impl LrcpStream {
    fn new() -> Self {
        LrcpStream {}
    }
}
