#![allow(unused)]
use super::protocol::*;
use super::state::*;
use crate::{Error, Result};
use core::net::SocketAddr;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{Duration, interval};
use tokio_util::codec::Framed;
use tracing::{error, info};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ClientId {
    id: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct Client {
    pub client_id: ClientId,
    pub role: ClientRole,
    pub sender: mpsc::UnboundedSender<Message>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ClientRole {
    Undefined,
    Camera { road: u16, mile: u16, limit: u16 },
    Dispatcher { roads: Vec<u16> },
}

// Only compare client_id
impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.client_id == other.client_id
    }
}

#[derive(Debug)]
pub struct ClientChannel {
    pub client_id: ClientId,
    pub receiver: mpsc::UnboundedReceiver<Message>,
    pub sender: mpsc::UnboundedSender<Message>,
}

impl ClientId {
    pub fn new(id: SocketAddr) -> Self {
        Self { id }
    }
}

impl Client {
    pub async fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|_| Error::General("Client disconnected".into()))
    }
}

impl ClientChannel {
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }

    pub async fn send(&mut self, msg: Message) -> Result<()> {
        let _ = self
            .sender
            .send(msg)
            .map_err(|e| Error::General(e.to_string()))?;
        Ok(())
    }
}

enum HeartbeatStatus {
    NotStarted,
    Running { cancel: oneshot::Sender<()> },
    Disabled,
}

struct ClientState {
    id: ClientId,
    role: ClientRole,
    heartbeat: HeartbeatStatus,
}

pub async fn handle_client(
    client_id: ClientId,
    state_tx: StateTx,
    socket: TcpStream,
) -> Result<()> {
    info!("handle_client: {:?}", client_id);
    let (mut sink, mut stream) = Framed::new(socket, MessageCodec::new()).split();

    let mut client_channel = state_tx.join(client_id.clone())?;
    let mut client_status = ClientState {
        id: client_id.clone(),
        role: ClientRole::Undefined,
        heartbeat: HeartbeatStatus::NotStarted,
    };

    loop {
        tokio::select! {
            msg = stream.next() => match msg {
                Some(Ok(msg)) => {
                    let _ = handle_client_socket_message(&mut client_status, &mut client_channel, &state_tx, msg).await?;
                }
                Some(Err(e)) => {
                    error!("Error reading message {}", e);
                    break;
                }
                None => {
                    break;
                }
            },
            Some(msg) = client_channel.recv() => {
                let _ = handle_message_from_client_channel(&state_tx, msg, &mut sink).await?;
            }
        }
    }

    let _ = state_tx.leave(client_id.clone())?;
    info!("client_id: {client_id:?} disconnect");

    Ok(())
}

type ClientSink = SplitSink<Framed<TcpStream, MessageCodec>, Message>;

async fn handle_message_from_client_channel(
    state: &StateTx,
    msg: Message,
    sink: &mut ClientSink,
) -> Result<()> {
    match msg {
        Message::Heartbeat => {
            let _ = sink.send(Message::Heartbeat).await;
        }
        Message::Ticket {
            plate,
            road,
            mile1,
            timestamp1,
            mile2,
            timestamp2,
            speed,
        } => {
            let _ = sink
                .send(Message::Ticket {
                    plate,
                    road,
                    mile1,
                    timestamp1,
                    mile2,
                    timestamp2,
                    speed,
                })
                .await;
        }
        Message::Error { msg } => {
            let _ = sink.send(Message::Error { msg }).await;
            return Err(Error::General(
                "disconnect after sending error message to client".into(),
            ));
        }
        other => {
            return Err(Error::General(format!(
                "other message should not be sent to client, msg: {:?}",
                other
            )));
        }
    }
    Ok(())
}

async fn handle_client_socket_message(
    client: &mut ClientState,
    client_channel: &mut ClientChannel,
    state: &StateTx,
    msg: Message,
) -> Result<()> {
    match msg {
        Message::IAmCamera { road, mile, limit } => {
            let _ = state.send(Message::SetRole {
                client_id: client.id.clone(),
                role: ClientRole::Camera { road, mile, limit },
            })?;
        }
        Message::IAmDispatcher { numroads: _, roads } => {
            let _ = state.send(Message::SetRole {
                client_id: client.id.clone(),
                role: ClientRole::Dispatcher { roads },
            })?;
        }
        Message::Plate { plate, timestamp } => {
            let _ = state.send(Message::PlateEvent {
                client_id: client.id.clone(),
                plate: plate.clone().into(),
                timestamp,
            })?;
        }
        Message::WantHeartbeat { interval } => {
            // Enforce: only once (or allow reconfigure?)
            if !matches!(client.heartbeat, HeartbeatStatus::NotStarted) {
                // Per spec: multiple WantHeartbeat = error → close connection
                let _ = client_channel.send(Message::Error {
                    msg: "Duplicate WantHeartbeat".into(),
                });
            }

            if interval == 0 {
                client.heartbeat = HeartbeatStatus::Disabled;
            } else {
                let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
                start_heartbeat_task(client_channel, interval, cancel_rx).await;
                client.heartbeat = HeartbeatStatus::Running { cancel: cancel_tx };
            }
        }
        other => {
            error!("unexpected message from socket, msg: {:?}", other);
            return Err(Error::General(format!(
                "unexpected message from socket, msg: {:?}",
                other
            )));
        }
    }
    Ok(())
}

async fn start_heartbeat_task(
    client_channel: &ClientChannel,
    deciseconds: u32,
    mut cancel: oneshot::Receiver<()>,
) {
    let duration = Duration::from_millis(deciseconds as u64 * 100);
    let mut interval = interval(duration);
    let client_sender = client_channel.sender.clone();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Send heartbeat on tick
                _ = interval.tick() => {
                    if client_sender.send(Message::Heartbeat).is_err() {
                        break; // client gone
                    }
                }
               // Exit if cancellation signal received (or sender dropped)
                _ = &mut cancel => {
                    break;
                }
            }
        }
    });
}
