#![allow(unused)]
use super::protocol::*;
use bytes::Bytes;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::{Interval, interval};

#[derive(Debug)]
pub enum SessionCommand {
    /// App wants to write data to the stream
    Write {
        data: Vec<u8>,
        reply: oneshot::Sender<std::io::Result<usize>>,
    },
    /// App wants to read data (non-blocking poll)
    /// We'll use a different mechanism for AsyncRead (see LrcpStream)
    Shutdown,
}

#[derive(Debug)]
pub enum SessionEvent {
    /// From network: data packet
    Data { pos: u64, escaped_data: String },
    /// From network: ACK
    Ack { length: u64 },
    /// From network: close
    Close,
    /// Retransmit timer fired
    Retransmit,
    /// Idle timeout
    IdleTimeout,
}

pub struct Session {
    session_id: u64,
    peer: std::net::SocketAddr,
    udp_tx: mpsc::UnboundedSender<UdpPacket>,

    // Incoming stream
    recv_pos: u64,
    recv_buffer: Vec<u8>,

    // Outgoing stream
    send_pos: u64,
    acked_pos: u64,
    pending_data: Vec<u8>,

    last_activity: Instant,
    // ✅ New: channel to send received data to the application
    pub read_tx: mpsc::UnboundedSender<Bytes>,
}

#[derive(Debug)]
pub struct UdpPacket {
    pub target: std::net::SocketAddr,
    pub payload: Vec<u8>,
}

impl UdpPacket {
    pub fn new(target: std::net::SocketAddr, s: String) -> Self {
        Self {
            target,
            payload: s.into_bytes(),
        }
    }
}

impl Session {
    pub async fn spawn(
        session_id: u64,
        peer: std::net::SocketAddr,
        udp_tx: mpsc::UnboundedSender<UdpPacket>,
        mut cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
        mut event_rx: mpsc::UnboundedReceiver<SessionEvent>,
        read_tx: mpsc::UnboundedSender<Bytes>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut session = Self {
            session_id,
            peer,
            udp_tx,
            recv_pos: 0,
            recv_buffer: Vec::new(),
            send_pos: 0,
            acked_pos: 0,
            pending_data: Vec::new(),
            last_activity: Instant::now(),
            read_tx,
        };

        // Send initial ACK
        session.send_ack().await;

        // Timers
        let mut retransmit_interval = interval(Duration::from_secs(3));
        let mut idle_check = interval(Duration::from_secs(10)); // check every 10s

        loop {
            tokio::select! {
                // Command from LrcpStream (app)
                Some(cmd) = cmd_rx.recv() => {
                    session.handle_command(cmd).await?;
                }

                // Event from network or timer
                Some(event) = event_rx.recv() => {
                    session.handle_event(event).await?;
                }

                // Retransmit timer
                _ = retransmit_interval.tick() => {
                    session.handle_event(SessionEvent::Retransmit).await?;
                }

                // Idle check
                _ = idle_check.tick() => {
                    if session.last_activity.elapsed() > Duration::from_secs(60) {
                        break;
                    }
                }

                else => break,
            }
        }

        // Send close on exit
        let _ = session.udp_tx.send(UdpPacket::new(
            session.peer,
            format!("/close/{}/", session.session_id),
        ));
        Ok(())
    }

    async fn handle_command(
        &mut self,
        cmd: SessionCommand,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match cmd {
            SessionCommand::Write { data, reply } => {
                self.pending_data.extend_from_slice(&data);
                self.send_pending_data().await;
                let _ = reply.send(Ok(data.len()));
            }
            SessionCommand::Shutdown => {
                // Graceful shutdown
            }
        }
        Ok(())
    }

    async fn handle_event(
        &mut self,
        event: SessionEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.last_activity = Instant::now();

        match event {
            SessionEvent::Data { pos, escaped_data } => {
                if pos == self.recv_pos {
                    let unescaped = unescape_data(&escaped_data);
                    let bytes = unescaped.as_bytes();
                    self.recv_buffer.extend_from_slice(bytes);
                    self.recv_pos += bytes.len() as u64;
                    self.send_ack().await;
                    // Note: we don't notify reader here — LrcpStream polls recv_buffer via channel
                } else {
                    // Request retransmission by re-acking current position
                    self.send_ack().await;
                }
            }

            SessionEvent::Ack { length } => {
                if length <= self.acked_pos {
                    // Duplicate ACK — ignore
                } else if length > self.send_pos {
                    // Misbehaving client
                    return Err("client acked more than sent".into());
                } else {
                    self.acked_pos = length;
                    // Truncate pending_data: everything before acked_pos is confirmed
                    let confirmed_bytes = (self.send_pos - self.acked_pos) as usize;
                    if confirmed_bytes < self.pending_data.len() {
                        self.pending_data
                            .drain(..(self.pending_data.len() - confirmed_bytes));
                    } else {
                        self.pending_data.clear();
                    }
                }
            }

            SessionEvent::Retransmit => {
                if !self.pending_data.is_empty() {
                    self.send_pending_data().await;
                }
            }

            SessionEvent::Close => {
                // Session closed by peer
                return Err("session closed by peer".into());
            }

            SessionEvent::IdleTimeout => {
                return Err("idle timeout".into());
            }
        }

        Ok(())
    }

    async fn send_ack(&self) {
        let ack = format!("/ack/{}/{}", self.session_id, self.recv_pos);
        let _ = self.udp_tx.send(UdpPacket::new(self.peer, ack));
    }

    async fn send_pending_data(&self) {
        if self.pending_data.is_empty() {
            return;
        }
        // Chunk to fit under 1000 bytes
        let max_payload_len = 900; // conservative
        let data = std::str::from_utf8(&self.pending_data).unwrap_or_default();
        let escaped = escape_data(data);
        let message = format!("/data/{}/{}/{}/", self.session_id, self.acked_pos, escaped);
        if message.len() < 1000 {
            let _ = self.udp_tx.send(UdpPacket::new(self.peer, message));
        } else {
            // TODO: chunking logic (split data into multiple /data/ messages)
            eprintln!("TODO: chunk large message");
        }
    }
}
