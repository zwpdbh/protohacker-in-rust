#![allow(unused)]
// https://protohackers.com/problem/6

use super::client::*;
use super::protocol::*;
use super::state::*;
use crate::{Error, Result};
use futures::TryStreamExt;
use futures::stream::SplitSink;
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Framed;
use tracing::error;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    let state = State::new();

    loop {
        let (socket, addr) = listener.accept().await?;
        let client_id = ClientId::new(addr);
        tokio::spawn(handle_client(client_id, state.clone(), socket));
    }
}

async fn handle_client(client_id: ClientId, state: State, socket: TcpStream) -> Result<()> {
    let (sink, mut stream) = Framed::new(socket, MessageCodec::new()).split();

    let mut client_handle = state.join(client_id.clone())?;

    loop {
        tokio::select! {
            Some(msg) = client_handle.recv() => {
                let _ = handle_state_message(&state, msg, &sink).await?;
            }
            msg = stream.next() => match msg {
                Some(Ok(msg)) => {
                    let _ = handle_client_socket_message(&client_id, &state, msg, &sink).await?;
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

    Ok(())
}

async fn handle_client_socket_message(
    client_id: &ClientId,
    state: &State,
    msg: Message,
    sink: &SplitSink<Framed<TcpStream, MessageCodec>, Message>,
) -> Result<()> {
    match msg {
        Message::IAmCamera { road, mile, limit } => {
            let _ = state.send(Message::SetRole {
                client_id: client_id.clone(),
                role: ClientRole::Camera { road, mile, limit },
            })?;
        }
        Message::IAmDispatcher { numroads: _, roads } => {
            let _ = state.send(Message::SetRole {
                client_id: client_id.clone(),
                role: ClientRole::Dispatcher { roads },
            })?;
        }
        Message::WantHeartbeat { interval } => {
            todo!()
        }
        _ => {
            todo!()
        }
    }
    Ok(())
}

async fn handle_state_message(
    state: &State,
    msg: Message,
    sink: &SplitSink<Framed<TcpStream, MessageCodec>, Message>,
) -> Result<()> {
    Ok(())
}

use tokio::time::{Duration, interval};

async fn start_heartbeat_task(
    client_id: ClientId,
    state: State,
    deciseconds: u32,
    sink: &SplitSink<Framed<TcpStream, MessageCodec>, Message>,
) {
    let duration = Duration::from_millis(deciseconds as u64 * 100); // 1 decisecond = 100 ms
    let mut interval = interval(duration);

    tokio::spawn(async move {
        loop {
            interval.tick().await;
            // Send Heartbeat to this client via state
            let _ = state.send(Message::Heartbeat);
        }
    });
}
