use super::user::OutgoingMessage;
use super::user::User;
use super::user::Username;
use crate::{Error, Result};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum RoomMessage {
    Chat {
        from: Username,
        text: String,
    },
    UserJoin {
        username: Username,
        client_ref: mpsc::UnboundedSender<OutgoingMessage>,
    },
    UserLeave {
        username: Username,
    },
}

#[derive(Debug, Clone)]
pub struct Room {
    // For client talk to room
    room_handle: mpsc::UnboundedSender<RoomMessage>,
}
impl Room {
    pub fn new() -> Room {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(run_room(rx));
        Room { room_handle: tx }
    }
}

impl Room {
    pub fn join(
        &self,
        username: Username,
        client_tx: mpsc::UnboundedSender<OutgoingMessage>,
    ) -> Result<()> {
        self.room_handle
            .send(RoomMessage::UserJoin {
                username,
                client_ref: client_tx,
            })
            .map_err(|_| Error::General("Room channel closed".into()))
    }

    pub fn leave(&self, username: Username) -> Result<()> {
        self.room_handle
            .send(RoomMessage::UserLeave { username })
            .map_err(|_| Error::General("Room channel closed".into()))
    }

    pub fn send_chat(&self, from: Username, text: String) -> Result<()> {
        self.room_handle
            .send(RoomMessage::Chat { from, text })
            .map_err(|_| Error::General("Room channel closed".into()))
    }
}

// a task which keep receiving ServerMessage and
// broadcast Message to different client
pub async fn run_room(mut rx: mpsc::UnboundedReceiver<RoomMessage>) -> Result<()> {
    // review: each client is represented by username with mpsc::UnboundedSender<Message>
    // which act like elixir's pid to allow you send message to it.
    let mut users: HashMap<Username, User> = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            RoomMessage::UserJoin {
                username,
                client_ref,
            } => {
                // 1. Send presence list to the NEW user
                let current_users: Vec<Username> = users.keys().cloned().collect();
                let _ = client_ref.send(OutgoingMessage::Participants(current_users));

                // 2. Notify ALL OTHER users that this user joined
                let join_msg = OutgoingMessage::UserJoin(username.clone());
                for (_, sender) in users.iter() {
                    let _ = sender.send(join_msg.clone());
                }

                // 3. Register new user
                users.insert(
                    username.clone(),
                    User {
                        client_handle: client_ref,
                    },
                );
            }
            RoomMessage::UserLeave { username } => {
                users.remove(&username);
                let leave_msg = OutgoingMessage::UserLeave(username.clone());

                for (_user, client_ref) in users.iter() {
                    let _ = client_ref.send(leave_msg.clone());
                }
            }
            RoomMessage::Chat { from, text } => {
                let chat_msg = OutgoingMessage::Chat {
                    from: from.clone(),
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
