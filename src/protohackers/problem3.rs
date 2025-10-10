// https://protohackers.com/problem/3
#![allow(unused)]

use std::fmt::{self, Display};

use crate::{Error, Result};
use bincode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

type User = String;

enum Session {
    Connected,
    WaitingUserResponse,
    UserJoined(User),
}

// represent all avaliable messages send to client
#[derive(Debug, Clone)]
enum Message {
    Chat(String),
    UserJoin(User),
    UserLeave(User),
    Welcome,
}

#[derive(Debug, Clone)]
enum ServerMessage {
    Chat {
        from: User,
        text: String,
    },
    UserJoin {
        username: String,
        sender: mpsc::UnboundedSender<Message>,
    },
    UserLeave {
        username: String,
    },
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Message::Chat(text) => text.to_string(),
            Message::Welcome => "Welcome to budgetchat! What shall I call you?".to_string(),
            Message::UserJoin(username) => {
                format!("{username} has entered the room")
            }
            Message::UserLeave(username) => format!("{username} has left the room"),
        };
        write!(f, "{}", output)
    }
}

async fn send<T: Display>(msg: T, output_stream: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
    // Write the bytes to the output stream
    output_stream.write_all(msg.to_string().as_bytes()).await?;
    // Ensure the data is flushed (optional but recommended for network streams)
    output_stream.flush().await?;
    Ok(())
}

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    let (manager_tx, manager_rx) = mpsc::unbounded_channel::<ServerMessage>();

    tokio::spawn(run_manager(manager_rx));

    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

// a task which keep receiving ServerMessage and
// broadcast Message to different client
async fn run_manager(mut rx: mpsc::UnboundedReceiver<ServerMessage>) -> Result<()> {
    // review: each client is represented by username with mpsc::UnboundedSender<Message>
    // which act like elixir's pid to allow you send message to it.
    let mut users: HashMap<String, mpsc::UnboundedSender<Message>> = HashMap::new();
    while let Some(msg) = rx.recv().await {
        match msg {
            ServerMessage::UserJoin { username, sender } => {
                let join_msg = Message::UserJoin(username.clone());
                for (_, s) in users.iter() {
                    let _ = s.send(join_msg.clone());
                }
                users.insert(username.clone(), sender);
            }
            ServerMessage::UserLeave { username } => {
                users.remove(&username);
                let leave_msg = Message::UserLeave(username);
                for (_, sender) in users.iter() {
                    let _ = sender.send(leave_msg.clone());
                }
            }
            ServerMessage::Chat { from, text } => {
                todo!()
            }
        }
    }
    Ok(())
}

async fn handle_client(mut socket: TcpStream) -> Result<()> {
    let (input_stream, output_stream) = socket.split();
    handle_client_internal(input_stream, output_stream).await;
    Ok(())
}

async fn handle_client_internal(
    input_stream: impl AsyncRead + Unpin,
    mut output_stream: impl AsyncWrite + Unpin,
) -> Result<()> {
    let mut session = Session::Connected;

    let input_stream = BufReader::new(input_stream);
    let mut lines = input_stream.lines();
    while let Some(line) = lines.next_line().await? {
        match session {
            Session::Connected => {
                // when the connection is established but user has not provided name
                // send hello to user to ask name
                let message = "Welcome to budgetchat! What shall I call you?";
                let _ = send(message, &mut output_stream).await?;

                session = Session::WaitingUserResponse
            }
            Session::WaitingUserResponse => {
                let username = get_valid_name(&line)?;
                session = Session::UserJoined(username)
            }
            Session::UserJoined(user) => {
                // forward user's message to other people.
                todo!("process message received from user")
            }
        }
    }
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
