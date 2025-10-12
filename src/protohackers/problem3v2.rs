// ref: https://enricorisa.me/blog/protohackers-budget-chat/
#![allow(unused)]

use crate::{Error, Result};
use futures::Sink;
use futures::SinkExt;
use futures::StreamExt;
use std::fmt;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_stream::Stream;
use tokio_util::codec::{Decoder, Encoder, Framed, LinesCodec};
use tracing::{debug, error, info};

#[derive(derive_more::Display, Clone)]
struct Username(String);

impl Username {
    pub fn parse(input: String) -> Result<Username> {
        if input.is_empty() {
            return Err(Error::General(
                "Name should be at least 1 character".to_string(),
            ));
        }
        if input.chars().any(|c| !c.is_alphanumeric()) {
            return Err(Error::General(
                "Name should contains only alphanumeric characters".to_string(),
            ));
        }
        Ok(Username(input))
    }
}

#[derive(derive_more::Display, Clone)]
enum OutgoingMessage {
    #[display("Welcome to budgetchat! What shall I call you?")]
    Welcome,

    #[display("* {} has entered the room", _0)]
    Join(Username),

    #[display("* {} has left the room", _0)]
    Leave(Username),

    #[display("[{}] {}", from, msg)]
    Chat { from: Username, msg: String },

    #[display("Invalid username {}", _0)]
    InvalidUsername(String),

    #[display("* The room contains: {}", "self.participants(_0)")]
    Participants(Vec<Username>),
}

impl OutgoingMessage {
    fn participants(&self, participants: &[Username]) -> String {
        participants
            .iter()
            .map(|user| user.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }
}

struct ChatCodec {
    lines: LinesCodec,
}

impl ChatCodec {
    fn new() -> Self {
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
            .map_err(|e| Error::General(e.to_string()))
    }
}

impl Decoder for ChatCodec {
    type Item = String;
    type Error = crate::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>> {
        self.lines
            .decode(src)
            .map_err(|e| Error::General(e.to_string()))
    }
}

pub async fn handle_client(state: (), mut stream: TcpStream, address: SocketAddr) -> Result<()> {
    let (input_stream, output_stream) = Framed::new(stream, ChatCodec::new()).split();

    handle_client_internal(state, address, input_stream, output_stream).await
}

async fn handle_client_internal<I, O>(
    state: (),
    address: SocketAddr,
    mut input_stream: O,
    mut out_stream: I,
) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<OutgoingMessage, Error = Error> + Unpin,
{
    let _ = input_stream.send(OutgoingMessage::Welcome).await?;
    Ok(())
}
