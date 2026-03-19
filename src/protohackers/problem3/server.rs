// https://protohackers.com/problem/3
// #![allow(unused)]

use super::protocol::*;
use super::room::*;
use crate::{Error, Result};

use crate::protohackers::HOST;
use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;
use tracing::error;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("{HOST}:{port}");
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
    // split() returns (Sink, Stream) - Sink for writing TO client, Stream for reading FROM client
    let (to_client, from_client) = Framed::new(stream, ChatCodec::new()).split();
    handle_client_internal(room, client_id, from_client, to_client).await
}

/// Handles a single client connection with bidirectional message flow.
///
/// Message flow:
/// - `from_client`: Stream of messages FROM the client (user input)
/// - `to_client`: Sink for messages TO the client (broadcasts from room)
/// - `room_inbox`: Channel for sending messages TO the room manager
async fn handle_client_internal<I, O>(
    room: Room,
    client_id: ClientId,
    mut from_client: I,
    mut to_client: O,
) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<OutgoingMessage, Error = Error> + Unpin,
{
    // 1. Send welcome message
    to_client.send(OutgoingMessage::Welcome).await?;

    // 2. Get username from the first line
    let username = from_client
        .try_next()
        .await?
        .ok_or_else(|| Error::Other("Error while waiting for the username".into()))?;

    let username = match Username::parse(&username) {
        Ok(username) => username,
        Err(e) => {
            to_client
                .send(OutgoingMessage::InvalidUsername(e.to_string()))
                .await?;
            return Ok(());
        }
    };

    // 3. Join the room - returns a channel for receiving broadcasts from other users
    let mut room_broadcasts = room.join(client_id.clone(), username.clone())?;

    // 4. Main event loop: handle both directions concurrently
    loop {
        tokio::select! {
            // Direction 1: Receive broadcast FROM room → forward TO client
            Some(msg) = room_broadcasts.recv() => {
                if let Err(e) = to_client.send(msg).await {
                    error!("Error sending message to client: {}", e);
                    break;
                }
            }

            // Direction 2: Receive message FROM client → forward TO room for broadcast
            result = from_client.next() => match result {
                Some(Ok(msg)) => {
                    if let Err(e) = room.broadcast_message(client_id.clone(), msg).await {
                        error!("Error broadcasting message: {}", e);
                    }
                }
                Some(Err(e)) => {
                    error!("Error reading message from client: {}", e);
                    break;
                }
                None => {
                    // Client disconnected
                    break;
                }
            }
        }
    }

    // 5. Notify room that user has left
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

    /// Test helper representing a connected client.
    /// Simulates the two directions of message flow:
    /// - `received_from_server`: Channel for messages sent TO the client
    /// - `send_to_server`: Channel for messages sent FROM the client
    struct TestClient {
        received_from_server: Receiver<OutgoingMessage>,
        send_to_server: Option<Sender<Result<String>>>,
        handle: JoinHandle<Result<()>>,
    }

    async fn connect(room: Room, client_id: ClientId) -> TestClient {
        // Channel for server → client (messages the client receives)
        let (to_client_tx, to_client_rx) = mpsc::channel(100);

        // Channel for client → server (messages the client sends)
        let (from_client_tx, mut from_client_rx) = mpsc::channel(100);

        let from_client_stream = async_stream::stream! {
            while let Some(message) = from_client_rx.recv().await {
                yield message
            }
        };

        // Make sender compatible with `Sink` trait
        let to_client_sink =
            PollSender::new(to_client_tx).sink_map_err(|e| Error::Other(e.to_string()));

        let handle = tokio::spawn(async move {
            handle_client_internal(
                room,
                client_id,
                Box::pin(from_client_stream),
                to_client_sink,
            )
            .await
        });

        TestClient {
            received_from_server: to_client_rx,
            send_to_server: Some(from_client_tx),
            handle,
        }
    }

    impl TestClient {
        /// Simulate the client sending a message to the server.
        async fn send(&mut self, message: &str) {
            self.send_to_server
                .as_ref()
                .unwrap()
                .send(Ok(message.to_string()))
                .await
                .unwrap();
        }

        /// Simulate the client disconnecting.
        async fn disconnect(mut self) {
            let sender = self.send_to_server.take();
            drop(sender); // Closing the channel signals EOF to the server

            self.handle.await.unwrap().unwrap()
        }

        /// Assert that the client received a specific message from the server.
        async fn expect_message(&mut self, msg: OutgoingMessage) {
            assert_eq!(self.received_from_server.recv().await.unwrap(), msg);
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
        alice.expect_message(OutgoingMessage::Welcome).await;

        // alice sends the username and get the participants list
        alice.send(&alice_username.to_string()).await;
        alice
            .expect_message(OutgoingMessage::Participants(vec![]))
            .await;

        // bob connects
        let mut bob = connect(room.clone(), bob_client).await;
        bob.expect_message(OutgoingMessage::Welcome).await;

        // bob sends the username and get the participants list
        bob.send(&bob_username.to_string()).await;
        bob.expect_message(OutgoingMessage::Participants(vec![alice_username.clone()]))
            .await;

        // alice gets the notification of bob joining the room
        alice
            .expect_message(OutgoingMessage::UserJoin(bob_username.clone()))
            .await;

        // alice sends a message
        alice.send("Hi bob!").await;

        // bob gets alice's message
        bob.expect_message(OutgoingMessage::Chat {
            text: "Hi bob!".to_string(),
            from: alice_username.clone(),
        })
        .await;

        // bob sends a message
        bob.send("Hi alice!").await;

        // alice gets bob's message
        alice
            .expect_message(OutgoingMessage::Chat {
                text: "Hi alice!".to_string(),
                from: bob_username.clone(),
            })
            .await;

        // bob disconnects
        bob.disconnect().await;

        // alice gets the notification of bob leaving the room
        alice
            .expect_message(OutgoingMessage::UserLeave(bob_username))
            .await;

        Ok(())
    }
}
