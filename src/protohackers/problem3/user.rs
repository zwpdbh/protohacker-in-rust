use super::protocol::*;
use crate::{Error, Result};
use tokio::sync::mpsc;

/// Represents a user in the room with their outbound message channel.
#[derive(Debug, Clone)]
pub struct User {
    pub username: Username,
    pub sender: mpsc::UnboundedSender<OutgoingMessage>,
}

impl User {
    pub fn send(&self, msg: OutgoingMessage) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|_| Error::Other("Client disconnected".into()))
    }
}

/// Channel receiver for broadcasts from the room.
/// This is given to the client handler to receive messages from other users.
pub struct BroadcastReceiver {
    pub receiver: mpsc::UnboundedReceiver<OutgoingMessage>,
}

impl BroadcastReceiver {
    /// Receive a broadcast message from the room.
    pub async fn recv(&mut self) -> Option<OutgoingMessage> {
        self.receiver.recv().await
    }
}
