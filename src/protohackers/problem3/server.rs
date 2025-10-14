// https://protohackers.com/problem/3
// #![allow(unused)]

use super::room::*;
use super::user::*;
use crate::{Error, Result};
use core::net::SocketAddr;
use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Decoder, Encoder, Framed, LinesCodec};
use tracing::error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientId {
    id: SocketAddr,
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

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    let room = Room::new();
    loop {
        let (socket, addr) = listener.accept().await?;
        let client_id = ClientId { id: addr };
        // tokio::spawn(handle_client(socket, room.clone()));
        tokio::spawn(handle_client(room.clone(), socket, client_id));
    }
}

async fn handle_client(room: Room, stream: TcpStream, client_id: ClientId) -> Result<()> {
    let (input_stream, output_stream) = Framed::new(stream, ChatCodec::new()).split();
    handle_client_internal(room, client_id, input_stream, output_stream).await
}

async fn handle_client_internal<I, O>(
    room: Room,
    client_id: ClientId,
    mut sink: O,
    mut stream: I,
) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<OutgoingMessage, Error = Error> + Unpin,
{
    // review: use into_split to consumes the socket and returns owned
    // ReadHalf and WriteHalf, which can be moved into async tasks.
    // let (input_stream, mut output_stream) = socket.into_split();

    // 1. send welcome to client
    let _ = sink.send(OutgoingMessage::Welcome).await?;

    // 2. get username from the first line received from client
    let username = stream
        .try_next()
        .await?
        .ok_or_else(|| Error::General("Error while waiting for the username".into()))?;

    let username = match Username::parse(&username) {
        Ok(username) => username,
        Err(e) => {
            sink.send(OutgoingMessage::InvalidUsername(e.to_string()))
                .await?;
            return Ok(());
        }
    };

    // let (client_tx, mut client_rx) = mpsc::unbounded_channel::<OutgoingMessage>();

    // 3. send to manager that user has joined
    let mut user_handle = room.join(client_id.clone(), username.clone())?;

    loop {
        tokio::select! {
            // 4a. Receive message from manager → send to client
            Some(msg) = user_handle.recv() => {
                if let Err(e) = sink.send(msg).await {
                    error!("Error sending message {}",e);
                    break;
                }
            }

             // 4b. send message for broadcast
             result = stream.next() => match result {
                Some(Ok(msg)) => {
                    let _ = user_handle.send_chat_message(msg, &room).await;
                }
                Some(Err(e)) => {
                    error!("Error reading message {}", e);
                    break;
                }
                None => {
                    break;
                }
             }
        }
    }

    // 5. One EOF, notify manager user leave
    let _ = room.leave(client_id.clone());

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use tokio::{sync::mpsc::Receiver, task::JoinHandle};

    use super::*;

    struct UserTest {
        // sink_receiver: Receiver<OutgoingMessage>,
        // stream_sender: Option<Sender<Result<String>>>,
        // handle: JoinHandle<Result<()>>,
    }

    async fn connect(room: Room, client_id: &str) -> UserTest {
        todo!()
    }

    #[tokio::test]
    async fn example_session_test() -> Result<()> {
        let room = Room::new();
        let alice = Username::parse("alice")?;
        let bob = Username::parse("bob")?;

        Ok(())
    }
}
