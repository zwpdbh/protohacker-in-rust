// ref: https://enricorisa.me/blog/protohackers-budget-chat/

use crate::{Error, Result};
use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpStream,
    sync::{Mutex, mpsc},
};
use tokio_util::codec::{Decoder, Encoder, Framed, LinesCodec};
use tracing::error;

#[derive(derive_more::Display, Clone, Debug, PartialEq)]
struct Username(String);

impl Username {
    pub fn parse(input: String) -> Result<Username> {
        if input.is_empty() {
            return Err(Error::Other(
                "Name should be at least 1 character".to_string(),
            ));
        }
        if input.chars().any(|c| !c.is_alphanumeric()) {
            return Err(Error::Other(
                "Name should contains only alphanumeric characters".to_string(),
            ));
        }
        Ok(Username(input))
    }
}

#[derive(derive_more::Display, Clone, Debug, PartialEq)]
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

// Modeling the state

#[derive(Clone)]
pub struct Room(Arc<Mutex<HashMap<SocketAddr, User>>>);
struct User {
    username: Username,
    // used to send a message to a client
    sender: mpsc::UnboundedSender<OutgoingMessage>,
}

// Encapsulate all the actions that a user can do in a room.
// - send messages to other users
// - leave the room
struct UserHandle {
    username: Username,
    address: SocketAddr,
    // for receiving message from other participants
    receiver: mpsc::UnboundedReceiver<OutgoingMessage>,
    // the joined room
    room: Room,
}

impl UserHandle {
    async fn send_message(&self, msg: String) {
        self.room
            .broadcast(
                &self.address,
                OutgoingMessage::Chat {
                    from: self.username.clone(),
                    msg,
                },
            )
            .await
    }

    async fn leave(self) {
        self.room.leave(self.address).await
    }
}

impl Room {
    pub fn new() -> Room {
        Room(Arc::new(Mutex::new(HashMap::new())))
    }

    // When users join a room they should receive a notification with all the room members,
    // and the other users should receive the notification of the newly joined member.
    async fn join(&self, addr: SocketAddr, username: Username) -> UserHandle {
        // review: how sender is used for sending a OutgoingMessage to user.
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut users = self.0.lock().await;
        let existing_user_names = users
            .iter()
            .map(|(_, user)| user.username.clone())
            .collect::<Vec<Username>>();

        // send the user list notification
        let _ = sender.send(OutgoingMessage::Participants(existing_user_names));

        // broadcast the join message to the other users
        let _ = self
            .broadcast_internal(&addr, OutgoingMessage::Join(username.clone()), &mut users)
            .await;

        users.insert(
            addr,
            User {
                username: username.clone(),
                sender,
            },
        );

        UserHandle {
            username,
            receiver,
            room: self.clone(),
            address: addr,
        }
    }

    // method for broadcasting a message to the users excluding the one
    // identified by the addr
    async fn broadcast(&self, addr: &SocketAddr, msg: OutgoingMessage) {
        let mut users = self.0.lock().await;
        self.broadcast_internal(addr, msg, &mut users).await;
    }

    async fn leave(&self, addr: SocketAddr) {
        let mut users = self.0.lock().await;
        if let Some(leaving) = users.remove(&addr) {
            self.broadcast_internal(&addr, OutgoingMessage::Leave(leaving.username), &mut users)
                .await;
        }
    }

    async fn broadcast_internal(
        &self,
        addr: &SocketAddr,
        msg: OutgoingMessage,
        users: &mut HashMap<SocketAddr, User>,
    ) {
        for (user_addr, user) in users.iter_mut() {
            if addr != user_addr {
                let _ = user.sender.send(msg.clone());
            }
        }
    }
}

pub async fn handle_client(state: Room, stream: TcpStream, address: SocketAddr) -> Result<()> {
    let (input_stream, output_stream) = Framed::new(stream, ChatCodec::new()).split();

    handle_client_internal(state, address, input_stream, output_stream).await
}

async fn handle_client_internal<I, O>(
    state: Room,
    address: SocketAddr,
    mut sink: O,
    mut stream: I,
) -> Result<()>
where
    I: Stream<Item = Result<String>> + Unpin,
    O: Sink<OutgoingMessage, Error = Error> + Unpin,
{
    // 1. send welcome to client
    let _ = sink.send(OutgoingMessage::Welcome).await?;

    // 2. get username from the first line received from client
    let username = stream
        .try_next()
        .await?
        .ok_or_else(|| Error::Other("Error while waiting for the username".into()))?;

    let username = match Username::parse(username) {
        Ok(username) => username,
        Err(e) => {
            sink.send(OutgoingMessage::InvalidUsername(e.to_string()))
                .await?;
            return Ok(());
        }
    };

    // 3. user has joined
    let mut handle = state.join(address, username).await;

    loop {
        tokio::select! {
             // 4a. Receive message from manager → send to client
             Some(msg) = handle.receiver.recv() => {
                // Send it to the connected client
                if let Err(e) = sink.send(msg).await {
                    error!("Error sending message {}",e);
                    break;
                }
             }

             // 4b. send message for broadcast
             result = stream.next() => match result {
                Some(Ok(msg)) => {
                    handle.send_message(msg).await;
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

    handle.leave().await;

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use tokio::sync::mpsc::{Receiver, Sender};
    use tokio::task::JoinHandle;
    use tokio_util::sync::PollSender;

    struct UserTest {
        sink_receiver: Receiver<OutgoingMessage>,
        stream_sender: Option<Sender<Result<String>>>,
        handle: JoinHandle<Result<()>>,
    }

    async fn connect(room: Room, addr: &str) -> UserTest {
        // channel for sending from server -> client
        let (sink_tx, sink_rx) = mpsc::channel(100);

        // channel for sending from client -> server
        let (stream_tx, mut stream_rx) = mpsc::channel(100);

        let address: SocketAddr = addr.parse().unwrap();

        // review: convert the channel into a `Stream`
        let stream = async_stream::stream! {
            while let Some(message) = stream_rx.recv().await {
                yield message
            }
        };
        // review: make sender compatible with `Sink` trait
        let sink = PollSender::new(sink_tx).sink_map_err(|e| Error::Other(e.to_string()));

        let handle = tokio::spawn(async move {
            handle_client_internal(room, address, sink, Box::pin(stream)).await
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
    async fn example_session_test() {
        let room = Room::new();

        let alice_username = Username::parse("alice".to_string()).unwrap();
        let bob_username = Username::parse("bob".to_string()).unwrap();

        // alice connects
        let mut alice = connect(room.clone(), "127.0.0.1:10").await;
        alice.check_message(OutgoingMessage::Welcome).await;

        // alice sends the username and get the participants list
        alice.send(&alice_username.to_string().as_ref()).await;
        alice
            .check_message(OutgoingMessage::Participants(vec![]))
            .await;

        // bob connects
        let mut bob = connect(room.clone(), "127.0.0.1:11").await;
        bob.check_message(OutgoingMessage::Welcome).await;

        // bob sends the username and get the participants list
        bob.send(&bob_username.to_string().as_ref()).await;
        bob.check_message(OutgoingMessage::Participants(vec![alice_username.clone()]))
            .await;

        // alice gets the notification of bob joining the room
        alice
            .check_message(OutgoingMessage::Join(bob_username.clone()))
            .await;

        // alice sends a message
        alice.send("Hi bob!").await;

        // bob gets alice's message
        bob.check_message(OutgoingMessage::Chat {
            msg: "Hi bob!".to_string(),
            from: alice_username.clone(),
        })
        .await;

        // bob sends a message
        bob.send("Hi alice!").await;

        // alice gets bob's message
        alice
            .check_message(OutgoingMessage::Chat {
                msg: "Hi alice!".to_string(),
                from: bob_username.clone(),
            })
            .await;

        // bob leaves the room
        bob.leave().await;

        // alice gets the notification of bob leaving the room
        alice
            .check_message(OutgoingMessage::Leave(bob_username))
            .await;
    }
}
