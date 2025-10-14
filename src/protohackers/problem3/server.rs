// https://protohackers.com/problem/3
// #![allow(unused)]

use super::room::*;
use super::user::*;
use crate::{Error, Result};
use std::fmt::Display;
use tokio::sync::mpsc::{self};
use tokio::{
    io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    let room = Room::new();
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(handle_client(socket, room.clone()));
    }
}

// review: to allow each client to send message to the each other,
// each client has a manager_tx: mpsc::UnboundedSender<ServerMessage>
// which allow client to send ServerMessage between tasks.
// Notice: to let client to send message to client
async fn handle_client(
    socket: TcpStream,
    // manager_tx: mpsc::UnboundedSender<RoomMessage>,
    room: Room,
) -> Result<()> {
    // review: use into_split to consumes the socket and returns owned
    // ReadHalf and WriteHalf, which can be moved into async tasks.
    let (input_stream, mut output_stream) = socket.into_split();

    // 1. send welcome to client
    let _ = send_to_client(OutgoingMessage::Welcome, &mut output_stream).await?;

    let input_stream = BufReader::new(input_stream);
    let mut lines = input_stream.lines();

    // 2. get username from the first line received from client
    let username = match lines.next_line().await? {
        Some(line) => Username::parse(&line)?,
        None => {
            return Err(Error::General(
                "Error while waiting for the username".into(),
            ));
        }
    };

    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<OutgoingMessage>();

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
