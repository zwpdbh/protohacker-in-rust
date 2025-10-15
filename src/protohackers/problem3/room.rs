use super::protocol::*;
use super::user::User;
use super::user::UserHandle;
use crate::{Error, Result};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum RoomMessage {
    Chat { from: ClientId, text: String },
    UserJoin { client_id: ClientId, user: User },
    UserLeave { client_id: ClientId },
}

#[derive(Debug, Clone)]
pub struct Room {
    sender: mpsc::UnboundedSender<RoomMessage>,
}

impl Room {
    pub fn new() -> Room {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(run_room(RoomHandle { receiver: rx }));
        Room { sender: tx }
    }

    pub fn join(&self, client_id: ClientId, username: Username) -> Result<UserHandle> {
        let (client_tx, client_rx) = mpsc::unbounded_channel::<OutgoingMessage>();

        let () = self
            .sender
            .send(RoomMessage::UserJoin {
                client_id: client_id.clone(),
                user: User {
                    username,
                    sender: client_tx,
                },
            })
            .map_err(|_| Error::General("Room channel closed".into()))?;

        return Ok(UserHandle {
            client_id: client_id.clone(),
            receiver: client_rx,
        });
    }

    pub fn leave(&self, client_id: ClientId) -> Result<()> {
        self.sender
            .send(RoomMessage::UserLeave { client_id })
            .map_err(|_| Error::General("Room channel closed".into()))
    }

    pub fn send_chat(&self, from: ClientId, text: String) -> Result<()> {
        self.sender
            .send(RoomMessage::Chat { from, text })
            .map_err(|_| Error::General("Room channel closed".into()))
    }
}

struct RoomHandle {
    receiver: mpsc::UnboundedReceiver<RoomMessage>,
}

impl RoomHandle {
    async fn recv(&mut self) -> Option<RoomMessage> {
        self.receiver.recv().await
    }
}

// a task which keep receiving ServerMessage and
// broadcast Message to different client
async fn run_room(mut room_handle: RoomHandle) -> Result<()> {
    // review: each client is represented by username with mpsc::UnboundedSender<Message>
    // which act like elixir's pid to allow you send message to it.
    let mut users: HashMap<ClientId, User> = HashMap::new();

    while let Some(msg) = room_handle.recv().await {
        match msg {
            RoomMessage::UserJoin { client_id, user } => {
                // 1. Send presence list to the NEW user
                let current_users: Vec<Username> = users
                    .values()
                    .into_iter()
                    .map(|v| v.username.clone())
                    .collect();
                let _ = user.send(OutgoingMessage::Participants(current_users));

                // 2. Notify ALL OTHER users that this user joined
                let join_msg = OutgoingMessage::UserJoin(user.username.clone());
                for (_, sender) in users.iter() {
                    let _ = sender.send(join_msg.clone());
                }

                // 3. Register new user
                users.insert(client_id, user);
            }
            RoomMessage::UserLeave { client_id } => {
                let user = users.remove(&client_id);
                let leave_msg = OutgoingMessage::UserLeave(user.unwrap().username);

                for (_user, client_ref) in users.iter() {
                    let _ = client_ref.send(leave_msg.clone());
                }
            }
            RoomMessage::Chat { from, text } => {
                let user = users.get(&from).unwrap();
                let chat_msg = OutgoingMessage::Chat {
                    from: user.username.clone(),
                    text,
                };
                for (user, client_ref) in users.iter() {
                    if *user != from {
                        let _ = client_ref.send(chat_msg.clone());
                    }
                }
            }
        }
    }
    Ok(())
}
