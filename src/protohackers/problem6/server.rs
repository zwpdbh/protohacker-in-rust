// https://protohackers.com/problem/6

use crate::{Error, Result};
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::{Decoder, Encoder, Framed, LinesCodec};

pub struct MessageCodec {
    inner: LinesCodec,
}

impl MessageCodec {
    pub fn new() -> Self {
        Self {
            inner: LinesCodec::new(),
        }
    }
}

#[derive(derive_more::Display, Clone, Debug)]
pub struct Username(String);

// represent all available messages sent to client
#[derive(derive_more::Display, Clone, Debug)]
pub enum Message {
    #[display("[{}]: {}", from, text)]
    Chat {
        from: Username,
        text: String,
    },
    General(String),
}

impl Encoder<Message> for MessageCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: Message, dst: &mut bytes::BytesMut) -> Result<()> {
        self.inner
            .encode(item.to_string(), dst)
            .map_err(|e| Error::General(e.to_string()))
    }
}

impl Decoder for MessageCodec {
    type Item = String;
    type Error = crate::Error;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>> {
        self.inner
            .decode(src)
            .map_err(|e| Error::General(e.to_string()))
    }
}

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(socket: TcpStream) -> Result<()> {
    let (sink, stream) = Framed::new(socket, MessageCodec::new()).split();
    let _ = handle_client_internal(sink, stream).await;
    Ok(())
}

async fn handle_client_internal<I, O>(mut sink: O, mut stream: I) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<Message, Error = Error> + Unpin,
{
    Ok(())
}
