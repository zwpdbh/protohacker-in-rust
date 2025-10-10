// https://protohackers.com/problem/3
#![allow(unused)]

use std::fmt;

use crate::{Error, Result};
use bincode;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize, Deserialize)]
enum Message {
    Chat(String),
    UserJoin(User),
    UserLeave(User),
    Welcome,
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Message::Welcome => "Welcome to budgetchat! What shall I call you?".to_string(),
            Message::Chat(s) => s.to_string(),
            Message::UserJoin(user) => format!("{user} has entered the room"),
            Message::UserLeave(user) => format!("{user} has left the room"),
        };
        write!(f, "{}", output)
    }
}

impl Message {
    async fn send(&self, output_stream: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
        // Convert the message to a string using Display
        let message_str = format!("{}\n", self);

        // Write the bytes to the output stream
        output_stream.write_all(message_str.as_bytes()).await?;

        // Ensure the data is flushed (optional but recommended for network streams)
        output_stream.flush().await?;
        Ok(())
    }
}

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
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
                let message = Message::Welcome;
                let _ = message.send(&mut output_stream).await?;
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

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use anyhow::{Ok, Result};

    #[test]
    fn parse_message() -> Result<()> {
        Ok(())
    }

    // #[tokio::test]
    // async fn test_name() -> Result<()> {
    //     Ok(())
    // }
}
