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
use tracing::info;

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
    pub receiver: mpsc::UnboundedReceiver<Message>,
    pub sender: mpsc::UnboundedSender<Message>,
}

impl ClientId {
    pub fn new(id: SocketAddr) -> Self {
        Self { id }
    }
}

impl Client {
    pub fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|_| Error::Other("Client disconnected".into()))
    }
}

impl ClientChannel {
    pub async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }

    pub fn send(&mut self, msg: Message) -> Result<()> {
        let _ = self
            .sender
            .send(msg)
            .map_err(|e| Error::Other(e.to_string()))?;
        Ok(())
    }
}

enum HeartbeatStatus {
    NotStarted,
    Running {
        #[allow(unused)]
        cancel: oneshot::Sender<()>,
    },
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
    let mut client_state = ClientState {
        id: client_id.clone(),
        role: ClientRole::Undefined,
        heartbeat: HeartbeatStatus::NotStarted,
    };

    loop {
        tokio::select! {
            msg = stream.next() => {
                 let _ = handle_client_socket_message(&mut client_state, &mut client_channel, &state_tx, msg).await?;
            }
            Some(msg) = client_channel.recv() => {
                let _ = handle_message_from_client_channel(&state_tx, msg, &mut sink).await?;
            }
        }
    }

    #[allow(unreachable_code)]
    let _ = state_tx.leave(client_id.clone())?;
    info!("client_id: {client_id:?} disconnect");

    Ok(())
}

type ClientSink = SplitSink<Framed<TcpStream, MessageCodec>, Message>;

async fn handle_message_from_client_channel(
    _state: &StateTx,
    msg: Message,
    sink: &mut ClientSink,
) -> Result<()> {
    match msg {
        Message::Error { msg } => {
            let _ = sink.send(Message::Error { msg }).await?;
            return Err(Error::Other(
                "disconnect after sending error message to client".into(),
            ));
        }
        Message::Heartbeat => {
            let _ = sink.send(Message::Heartbeat).await?;
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
                .await?;
        }
        other => {
            return Err(Error::Other(format!(
                "other message should not be sent to client, msg: {:?}",
                other
            )));
        }
    }
    Ok(())
}

async fn handle_client_socket_message(
    client_state: &mut ClientState,
    client_channel: &mut ClientChannel,
    state: &StateTx,
    msg: Option<Result<Message>>,
) -> Result<()> {
    match msg {
        None => return Err(Error::Other("client disconnected".into())),
        Some(Err(_e)) => {
            let _ = client_channel.send(Message::Error {
                msg: "bad message".into(),
            });
        }
        Some(Ok(msg)) => {
            match msg {
                Message::IAmCamera { road, mile, limit } => match client_state.role {
                    ClientRole::Undefined => {
                        client_state.role = ClientRole::Camera { road, mile, limit };
                        info!(
                            "client: {:?}, role: {:?}",
                            client_state.id, client_state.role
                        );
                    }

                    _ => {
                        let _ = client_channel.send(Message::Error {
                            msg: "role validation failed".into(),
                        });
                    }
                },
                Message::IAmDispatcher { numroads: _, roads } => match client_state.role {
                    ClientRole::Undefined => {
                        client_state.role = ClientRole::Dispatcher {
                            roads: roads.clone(),
                        };
                        info!(
                            "client: {:?}, role: {:?}",
                            client_state.id, client_state.role
                        );
                        let _ = state.send(Message::DispatcherObservation {
                            client_id: client_state.id.clone(),
                            roads,
                        })?;
                    }
                    _ => {
                        let _ = client_channel.send(Message::Error {
                            msg: "role validation failed".into(),
                        })?;
                    }
                },
                Message::Plate { plate, timestamp } => match client_state.role {
                    ClientRole::Camera { road, mile, limit } => {
                        let _ = state.send(Message::PlateObservation {
                            client_id: client_state.id.clone(),
                            road,
                            mile,
                            limit,
                            plate: plate.into(),
                            timestamp,
                        })?;
                    }
                    _ => {
                        let _ = client_channel.send(Message::Error {
                            msg: "only camera should receive plate event".into(),
                        })?;
                    }
                },
                Message::WantHeartbeat { interval } => {
                    // Enforce: only once (or allow reconfigure?)
                    if !matches!(client_state.heartbeat, HeartbeatStatus::NotStarted) {
                        // Per spec: multiple WantHeartbeat = error → close connection
                        let () = client_channel.send(Message::Error {
                            msg: "Duplicate WantHeartbeat".into(),
                        })?;
                    }

                    if interval == 0 {
                        client_state.heartbeat = HeartbeatStatus::Disabled;
                    } else {
                        // review: how use one-shot channel with object drop to automatically start the task
                        // once client is dropped, the heartbeat task will be signaled to stop
                        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
                        start_heartbeat_task(client_channel, interval, cancel_rx).await;
                        client_state.heartbeat = HeartbeatStatus::Running { cancel: cancel_tx };
                    }
                }
                other => {
                    let () = client_channel.send(Message::Error {
                        msg: format!("unexpected message from socket, msg: {:?}", other).into(),
                    })?;
                }
            }
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
