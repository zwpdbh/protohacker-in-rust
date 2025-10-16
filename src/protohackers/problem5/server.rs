use crate::{Error, Result};
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Decoder, Encoder, Framed, LinesCodec};
use tracing::error;
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

// represent all avaliable messages send to client
#[derive(derive_more::Display, Clone, Debug)]
pub enum Message {
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
    let address = format!("127.0.0.1:{port}");
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

const TONY_ACCOUNT: &str = "7YWHMfk9JZe0LM0g1ZauHuiSxhI";
const UPSTREAM_HOST: &str = "chat.protohackers.com";
const UPSTREAM_PORT: &str = "16963";

// https://protohackers.com/problem/5
// 1. every message I received from client_socket, I need to send it via budget_chat_socket
// 2. every message I received from budget_chat_socket, I need to send it to client_socket
// 3. inspect message, find the account and replace it with @tony_account.
// 4. do 1, and 2 in parallel
// 5 if connection in 1 or 2 has problem, close the both connection
async fn handle_client_internal<I, O>(mut client_sink: O, mut client_stream: I) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<Message, Error = Error> + Unpin,
{
    let upstream = TcpStream::connect(format!("{}:{}", UPSTREAM_HOST, UPSTREAM_PORT)).await?;

    let (mut upstream_sink, mut upstream_stream) =
        Framed::new(upstream, MessageCodec::new()).split();

    loop {
        tokio::select! {
            // Message from CLIENT -> rewrite -> send to UPSTREAM
            client_msg = client_stream.next() => {
                match client_msg {
                    Some(Ok(msg)) => {
                        let rewritten = rewritten_account(&msg);
                        if let Err(e) = upstream_sink.send(Message::General(rewritten)).await {
                            error!("failed to send to upstream: {}", e);
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        error!("client_stream error: {}", e);
                        break;
                    }
                    None => {
                        break;
                    }
                }
            }
            // Message from UPSTREAM -> rewritte -> sent to CLIENT
            upstream_msg = upstream_stream.next() => {
                match upstream_msg {
                    Some(Ok(msg)) => {
                        let rewritten = rewritten_account(&msg);
                        if let Err(e) = client_sink.send(Message::General(rewritten)).await {
                            error!("failed to send to the client: {}", e);
                            break;
                        }

                    }
                    Some(Err(e)) => {
                        error!("upstream_stream error: {}", e);
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn rewritten_account(msg: &str) -> String {
    // This pattern captures: (prefix)(address)(suffix)
    // where prefix is start or space, suffix is space or end
    let re = regex::Regex::new(r"(^| )7[0-9A-Za-z]{25,34}($| )").unwrap();
    re.replace_all(msg, TONY_ACCOUNT).into_owned()
}
