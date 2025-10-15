// https://protohackers.com/problem/3
// #![allow(unused)]

use super::protocol::*;
use super::room::*;
use crate::{Error, Result};

use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::error;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    let room = Room::new();
    loop {
        let (socket, addr) = listener.accept().await?;
        let client_id = ClientId::new(addr);
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
    use super::*;
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio::task::JoinHandle;
    use tokio_util::sync::PollSender;

    struct UserTest {
        sink_receiver: Receiver<OutgoingMessage>,
        stream_sender: Option<Sender<Result<String>>>,
        handle: JoinHandle<Result<()>>,
    }

    async fn connect(room: Room, client_id: ClientId) -> UserTest {
        let (sink_tx, sink_rx) = mpsc::channel(100);

        let (stream_tx, mut stream_rx) = mpsc::channel(100);

        let stream = async_stream::stream! {
            while let Some(message) = stream_rx.recv().await {
                yield message
            }
        };

        // review: make sender compatible with `Sink` trait
        let sink = PollSender::new(sink_tx).sink_map_err(|e| Error::General(e.to_string()));

        let handle = tokio::spawn(async move {
            handle_client_internal(room, client_id, sink, Box::pin(stream)).await
        });

        UserTest {
            sink_receiver: sink_rx,
            stream_sender: Some(stream_tx),
            handle,
        }
    }

    impl UserTest {
        async fn send(&mut self, message: &str) {
            self.stream_sender
                .as_ref()
                .unwrap()
                .send(Ok(message.to_string()))
                .await
                .unwrap();
        }

        async fn leave(mut self) {
            let stream = self.stream_sender.take();
            drop(stream);

            self.handle.await.unwrap().unwrap()
        }

        async fn check_message(&mut self, msg: OutgoingMessage) {
            assert_eq!(self.sink_receiver.recv().await.unwrap(), msg);
        }
    }

    #[tokio::test]
    async fn example_session_test() -> Result<()> {
        let room = Room::new();

        let alice_username = Username::parse("alice").unwrap();
        let bob_username = Username::parse("bob").unwrap();

        let alice_client = ClientId::new("127.0.0.1:10".parse().unwrap());
        let bob_client = ClientId::new("127.0.0.1:11".parse().unwrap());

        // alice connects
        let mut alice = connect(room.clone(), alice_client).await;
        alice.check_message(OutgoingMessage::Welcome).await;

        // alice sends the username and get the participants list
        alice.send(&alice_username.to_string().as_ref()).await;
        alice
            .check_message(OutgoingMessage::Participants(vec![]))
            .await;

        // bob connects
        let mut bob = connect(room.clone(), bob_client).await;
        bob.check_message(OutgoingMessage::Welcome).await;

        // bob sends the username and get the participants list
        bob.send(&bob_username.to_string().as_ref()).await;
        bob.check_message(OutgoingMessage::Participants(vec![alice_username.clone()]))
            .await;

        // alice gets the notification of bob joining the room
        alice
            .check_message(OutgoingMessage::UserJoin(bob_username.clone()))
            .await;

        // alice sends a message
        alice.send("Hi bob!").await;

        // bob gets alice's message
        bob.check_message(OutgoingMessage::Chat {
            text: "Hi bob!".to_string(),
            from: alice_username.clone(),
        })
        .await;

        // bob sends a message
        bob.send("Hi alice!").await;

        // alice gets bob's message
        alice
            .check_message(OutgoingMessage::Chat {
                text: "Hi alice!".to_string(),
                from: bob_username.clone(),
            })
            .await;

        // bob leaves the room
        bob.leave().await;

        // alice gets the notification of bob leaving the room
        alice
            .check_message(OutgoingMessage::UserLeave(bob_username))
            .await;

        Ok(())
    }
}
