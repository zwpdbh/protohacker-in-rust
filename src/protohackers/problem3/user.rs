use super::protocol::*;
use super::room::Room;
use crate::{Error, Result};
use tokio::sync::mpsc;

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

pub struct UserHandle {
    pub client_id: ClientId,
    pub receiver: mpsc::UnboundedReceiver<OutgoingMessage>,
}

impl UserHandle {
    pub async fn send_chat_message(&self, msg: String, room: &Room) -> Result<()> {
        room.send_chat(self.client_id.clone(), msg)
    }

    pub async fn recv(&mut self) -> Option<OutgoingMessage> {
        self.receiver.recv().await
    }
}
