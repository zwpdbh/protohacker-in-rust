use super::lrcp::*;
use std::net::SocketAddr;

pub struct ClientId {
    id: SocketAddr,
}

impl ClientId {
    pub fn new(id: SocketAddr) -> Self {
        ClientId { id }
    }
}

pub async fn handle_client(client_id: ClientId, socket: LrcpStream) {}
