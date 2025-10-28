use crate::{Error, Result};
use core::net::SocketAddr;
use tokio_util::codec::{Decoder, Encoder, LinesCodec};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientId {
    id: SocketAddr,
}

impl ClientId {
    pub fn new(id: SocketAddr) -> Self {
        Self { id }
    }
}

#[derive(derive_more::Display, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn parse(name: &str) -> Result<Username> {
        if name.is_empty() {
            return Err(Error::Other("Username must not be empty".into()));
        }
        if name.len() > 16 {
            return Err(Error::Other(
                "Username must be at most 16 characters".into(),
            ));
        }
        if !name.chars().all(|c| c >= ' ' && c <= '~') {
            return Err(Error::Other(
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
    Participants(Vec<Username>),
    #[display("Invalid username {}", _0)]
    InvalidUsername(String),
}

pub struct ChatCodec {
    lines: LinesCodec,
}

impl ChatCodec {
    pub fn new() -> Self {
        Self {
            lines: LinesCodec::new(),
        }
    }
}

impl Encoder<OutgoingMessage> for ChatCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut bytes::BytesMut) -> Result<()> {
        self.lines
            .encode(item.to_string(), dst)
            .map_err(|e| Error::Other(e.to_string()))
    }
}

impl Decoder for ChatCodec {
    type Item = String;
    type Error = crate::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>> {
        self.lines
            .decode(src)
            .map_err(|e| Error::Other(e.to_string()))
    }
}
