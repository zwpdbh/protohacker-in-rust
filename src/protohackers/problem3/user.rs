use super::room::Room;
use crate::{Error, Result, protohackers::problem3::server::ClientId};
use tokio::sync::mpsc;

#[derive(derive_more::Display, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn parse(name: &str) -> Result<Username> {
        if name.is_empty() {
            return Err(Error::General("Username must not be empty".into()));
        }
        if name.len() > 16 {
            return Err(Error::General(
                "Username must be at most 16 characters".into(),
            ));
        }
        if !name.chars().all(|c| c >= ' ' && c <= '~') {
            return Err(Error::General(
                "Username must contain only printable ASCII characters".into(),
            ));
        }
        Ok(Username(name.to_string()))
    }
}

// represent all avaliable messages send to client
#[derive(derive_more::Display, Clone, Debug, PartialEq)]
pub enum OutgoingMessage {
    #[display("[{}] {}", from, text)]
    Chat { from: Username, text: String },
    #[display("* {} has entered the room", _0)]
    UserJoin(Username),
    #[display("* {} has left the room", _0)]
    UserLeave(Username),
    #[display("Welcome to budgetchat! What shall I call you?")]
    Welcome,
    #[display("* The room contains: {}", "self.participants(_0)")]
    Participants(Vec<ClientId>),
    #[display("Invalid username {}", _0)]
    InvalidUsername(String),
}

#[derive(Debug, Clone)]
pub struct User {
    pub username: Username,
    pub sender: mpsc::UnboundedSender<OutgoingMessage>,
}

impl User {
    pub fn send(&self, msg: OutgoingMessage) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|_| Error::General("Client disconnected".into()))
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
