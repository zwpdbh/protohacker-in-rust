// https://protohackers.com/problem/3
// #![allow(unused)]

use std::fmt::Display;

use crate::{Error, Result};
use std::collections::HashMap;
use tokio::sync::mpsc::{self};
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
#[derive(derive_more::Display, Clone, Debug, PartialEq, Eq, Hash)]
struct User(String);

// represent all avaliable messages send to client
#[derive(derive_more::Display, Clone, Debug, PartialEq)]
enum Message {
    #[display("[{}] {}", from, text)]
    Chat { from: User, text: String },
    #[display("* {} has entered the room", _0)]
    UserJoin(User),
    #[display("* {} has left the room", _0)]
    UserLeave(User),
    #[display("Welcome to budgetchat! What shall I call you?")]
    Welcome,
    #[display("* The room contains: {}", "self.participants(_0)")]
    Participants(Vec<User>),
}

#[derive(Debug, Clone)]
enum RoomMessage {
    Chat {
        from: User,
        text: String,
    },
    UserJoin {
        username: User,
        client_ref: mpsc::UnboundedSender<Message>,
    },
    UserLeave {
        username: User,
    },
}

// For Clients to talk to the Room
#[derive(Clone)]
pub struct RoomHandle {
    tx: mpsc::UnboundedSender<RoomMessage>,
}

pub struct Room;
impl Room {
    pub fn new() -> RoomHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(run_room(rx));
        RoomHandle { tx }
    }
}

impl RoomHandle {
    fn join(&self, username: User, client_tx: mpsc::UnboundedSender<Message>) -> Result<()> {
        self.tx
            .send(RoomMessage::UserJoin {
                username,
                client_ref: client_tx,
            })
            .map_err(|_| Error::General("Room channel closed".into()))
    }

    fn leave(&self, username: User) -> Result<()> {
        self.tx
            .send(RoomMessage::UserLeave { username })
            .map_err(|_| Error::General("Room channel closed".into()))
    }

    fn send_chat(&self, from: User, text: String) -> Result<()> {
        self.tx
            .send(RoomMessage::Chat { from, text })
            .map_err(|_| Error::General("Room channel closed".into()))
    }
}

// For Room to talk to a Client
#[derive(Clone)]
struct ClientHandle {
    tx: mpsc::UnboundedSender<Message>,
}

impl ClientHandle {
    fn send(&self, msg: Message) -> Result<()> {
        self.tx
            .send(msg)
            .map_err(|_| Error::General("Client disconnected".into()))
    }
}

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    let room_handle = Room::new();
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(handle_client(socket, room_handle.clone()));
    }
}

// a task which keep receiving ServerMessage and
// broadcast Message to different client
async fn run_room(mut rx: mpsc::UnboundedReceiver<RoomMessage>) -> Result<()> {
    // review: each client is represented by username with mpsc::UnboundedSender<Message>
    // which act like elixir's pid to allow you send message to it.
    let mut users: HashMap<User, ClientHandle> = HashMap::new();

    while let Some(msg) = rx.recv().await {
        match msg {
            RoomMessage::UserJoin {
                username,
                client_ref,
            } => {
                // 1. Send presence list to the NEW user
                let current_users: Vec<User> = users.keys().cloned().collect();
                let _ = client_ref.send(Message::Participants(current_users));

                // 2. Notify ALL OTHER users that this user joined
                let join_msg = Message::UserJoin(username.clone());
                for (_, sender) in users.iter() {
                    let _ = sender.send(join_msg.clone());
                }

                // 3. Register new user
                users.insert(username.clone(), ClientHandle { tx: client_ref });
            }
            RoomMessage::UserLeave { username } => {
                users.remove(&username);
                let leave_msg = Message::UserLeave(username.clone());

                for (_user, client_ref) in users.iter() {
                    let _ = client_ref.send(leave_msg.clone());
                }
            }
            RoomMessage::Chat { from, text } => {
                let chat_msg = Message::Chat {
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

// review: to allow each client to send message to the each other,
// each client has a manager_tx: mpsc::UnboundedSender<ServerMessage>
// which allow client to send ServerMessage between tasks.
// Notice: to let client to send message to client
async fn handle_client(
    socket: TcpStream,
    // manager_tx: mpsc::UnboundedSender<RoomMessage>,
    room: RoomHandle,
) -> Result<()> {
    // review: use into_split to consumes the socket and returns owned
    // ReadHalf and WriteHalf, which can be moved into async tasks.
    let (input_stream, mut output_stream) = socket.into_split();

    // 1. send welcome to client
    let _ = send_to_client(Message::Welcome, &mut output_stream).await?;

    let input_stream = BufReader::new(input_stream);
    let mut lines = input_stream.lines();

    // 2. get username from the first line received from client
    let username = match lines.next_line().await? {
        Some(line) => User(get_valid_name(&line)?),
        None => {
            return Err(Error::General(
                "Error while waiting for the username".into(),
            ));
        }
    };

    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<Message>();

    // 3. send to manager that user has joined
    let _ = room.join(username.clone(), client_tx)?;

    loop {
        tokio::select! {
            // 4a. Receive message from manager → send to client
            Some(msg) = client_rx.recv() => {
                if send_to_client(msg, &mut output_stream).await.is_err() {
                    break; // client disconnected
                }
            }

            // 4b. Read message from client → send to manager
            line_result = lines.next_line() => match line_result {
                Ok(Some(line)) => {
                    room.send_chat(username.clone(), line)?;
                }
                Ok(None) => break, // EOF
                Err(e) => {
                    eprintln!("Read error: {}", e);
                    break;
                }
            }
        }
    }

    // 5. One EOF, notify manager user leave
    let _ = room.leave(username.clone());

    Ok(())
}

async fn send_to_client<T: Display>(
    msg: T,
    output_stream: &mut (impl AsyncWrite + Unpin),
) -> Result<()> {
    // Write the bytes to the output stream
    output_stream
        .write_all(format!("{}\n", msg).as_bytes())
        .await?;
    // Ensure the data is flushed (optional but recommended for network streams)
    output_stream.flush().await?;
    Ok(())
}

fn get_valid_name(name: &str) -> Result<String> {
    if name.is_empty() {
        return Err(Error::General("Username must not be empty".into()));
    }
    if name.len() > 16 {
        return Err(Error::General(
            "Username must be at most 16 characters".into(),
        ));
    }
    if !name.chars().all(|c| c >= ' ' && c <= '~') {
        return Err(Error::General(
            "Username must contain only printable ASCII characters".into(),
        ));
    }
    Ok(name.to_string())
}
