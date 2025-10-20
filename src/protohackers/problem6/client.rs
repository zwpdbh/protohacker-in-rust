#![allow(unused)]
use super::protocol::*;
use crate::{Error, Result};
use core::net::SocketAddr;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ClientId {
    id: SocketAddr,
}

#[derive(Debug)]
pub struct Client {
    pub client_id: ClientId,
    pub role: ClientRole,
    pub sender: mpsc::UnboundedSender<Message>,
}

#[derive(Debug, PartialEq)]
pub enum ClientRole {
    Undefined,
    Camera { road: u16, mile: u16, limit: u16 },
    Dispatcher { roads: Vec<u16> },
}

// Only compare client_id
impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }
}

#[derive(Debug)]
pub struct ClientHandle {
    pub client_id: ClientId,
    pub receiver: mpsc::UnboundedReceiver<Message>,
}

impl ClientId {
    pub fn new(id: SocketAddr) -> Self {
        Self { id }
    }
}

impl Client {
    pub async fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|_| Error::General("Client disconnected".into()))
    }
}

impl ClientHandle {
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
}
