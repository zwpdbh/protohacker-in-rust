// https://protohackers.com/problem/3
// #![allow(unused)]

use std::fmt::{self, Display};

use crate::{Error, Result};
use std::collections::HashMap;
use tokio::sync::mpsc::{self};
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

type User = String;

// represent all avaliable messages send to client
#[derive(Debug, Clone)]
enum Message {
    Chat { from: User, text: String },
    UserJoin(User),
    UserLeave(User),
    Welcome,
    PresenceList(Vec<User>),
}

#[derive(Debug, Clone)]
enum ServerMessage {
    Chat {
        from: User,
        text: String,
    },
    UserJoin {
        username: String,
        client_ref: mpsc::UnboundedSender<Message>,
    },
    UserLeave {
        username: String,
    },
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Message::Chat { from, text } => format!("[{from}] {text}"),
            Message::Welcome => "Welcome to budgetchat! What shall I call you?".to_string(),
            Message::UserJoin(username) => {
                format!("{username} has entered the room")
            }
            Message::UserLeave(username) => format!("{username} has left the room"),
            Message::PresenceList(users) => {
                if users.is_empty() {
                    "* The room is empty".to_string()
                } else {
                    let list = users.join(", ");
                    format!("* The room contains: {}", list)
                }
            }
        };
        write!(f, "{}", output)
    }
}

async fn send<T: Display>(msg: T, output_stream: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
    // Write the bytes to the output stream
    output_stream
        .write_all(format!("{}\n", msg).as_bytes())
        .await?;
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
        tokio::spawn(handle_client(socket, manager_tx.clone()));
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
            ServerMessage::UserJoin {
                username,
                client_ref,
            } => {
                // 1. Send presence list to the NEW user
                let current_users: Vec<String> = users.keys().cloned().collect();
                let _ = client_ref.send(Message::PresenceList(current_users));

                // 2. Notify ALL OTHER users that this user joined
                let join_msg = Message::UserJoin(username.clone());
                for (_, sender) in users.iter() {
                    let _ = sender.send(join_msg.clone());
                }

                // 3. Register new user
                users.insert(username.clone(), client_ref);
            }
            ServerMessage::UserLeave { username } => {
                users.remove(&username);
                let leave_msg = Message::UserLeave(username.clone());

                for (_user, client_ref) in users.iter() {
                    let _ = client_ref.send(leave_msg.clone());
                }
            }
            ServerMessage::Chat { from, text } => {
                let chat_msg = Message::Chat {
                    from: from.clone(),
                    text,
                };
                for (user, client_ref) in users.iter() {
                    if user.to_string() != from {
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
    manager_tx: mpsc::UnboundedSender<ServerMessage>,
) -> Result<()> {
    // review: use into_split to consumes the socket and returns owned
    // ReadHalf and WriteHalf, which can be moved into async tasks.
    let (input_stream, mut output_stream) = socket.into_split();

    // 1. send welcome to client
    let _ = send(Message::Welcome, &mut output_stream).await?;

    let input_stream = BufReader::new(input_stream);
    let mut lines = input_stream.lines();

    // 2. get username from the first line received from client
    let username = match lines.next_line().await? {
        Some(line) => get_valid_name(&line)?,
        None => return Ok(()),
    };

    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<Message>();

    // 3. send to manager that user has joined
    let _ = manager_tx
        .send(ServerMessage::UserJoin {
            username: username.clone(),
            client_ref: client_tx,
        })
        .map_err(|e| Error::General(format!("{e}")))?;

    // 4a. recv message from manager and forward it to client socket
    let mut writer = output_stream;
    let forward_msg_to_manager_task = tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            let _ = send(msg, &mut writer);
        }
    });

    // 4b. reach chat message from client socket and forward to manager
    // let mut current_user = username;
    while let Some(line) = lines.next_line().await? {
        let _ = manager_tx
            .send(ServerMessage::Chat {
                from: username.clone(),
                text: line,
            })
            .map_err(|e| Error::General(format!("{e}")))?;
    }

    // 5. One EOF, notify manager user leave
    let _ = manager_tx.send(ServerMessage::UserLeave {
        username: username.clone(),
    });

    forward_msg_to_manager_task.abort();

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
