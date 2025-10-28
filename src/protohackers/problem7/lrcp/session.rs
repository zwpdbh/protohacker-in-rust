use super::protocol::*;
use bytes::Bytes;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::interval;
use tracing::debug;

#[derive(Debug)]
pub enum SessionCommand {
    /// App wants to write data to the stream
    Write {
        data: Vec<u8>,
        reply: oneshot::Sender<std::io::Result<usize>>,
    },
    /// App wants to read data (non-blocking poll)
    /// We'll use a different mechanism for AsyncRead (see LrcpStream)
    #[allow(unused)]
    Shutdown,
}

/// A session is a logical connection established with a UDP socket.
/// This event represent LRCP transport layer event for session
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

/// Manage the state of a single logical connection
pub struct Session {
    session_id: u64,
    peer: std::net::SocketAddr,
    udp_packet_pair_tx: mpsc::UnboundedSender<UdpPacketPair>,

    // Incoming stream
    in_pos: u64,
    in_buffer: Vec<u8>,

    // Outgoing stream
    out_pos: u64,
    acked_out_pos: u64,
    pending_out_data: Vec<u8>,

    last_activity: Instant,
    // ✅ New: channel to send received data to the application
    #[allow(unused)]
    bytes_tx: mpsc::UnboundedSender<Bytes>,
}

#[derive(Debug)]
pub struct UdpPacketPair {
    pub target: std::net::SocketAddr,
    pub payload: Vec<u8>,
}

impl UdpPacketPair {
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
        udp_packet_pair_tx: mpsc::UnboundedSender<UdpPacketPair>,
        mut session_cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
        mut session_event_rx: mpsc::UnboundedReceiver<SessionEvent>,
        bytes_tx: mpsc::UnboundedSender<Bytes>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut session = Self {
            session_id,
            peer,
            udp_packet_pair_tx,
            in_pos: 0,
            in_buffer: Vec::new(),
            out_pos: 0,
            acked_out_pos: 0,
            pending_out_data: Vec::new(),
            last_activity: Instant::now(),
            bytes_tx,
        };

        // Send initial ACK
        session.send_ack(0).await;

        // Timers
        let mut retransmit_interval = interval(Duration::from_secs(3));
        let mut idle_check = interval(Duration::from_secs(10)); // check every 10s

        loop {
            tokio::select! {
                // Command from LrcpStream (app)
                Some(cmd) = session_cmd_rx.recv() => {
                    session.handle_command(cmd).await?;
                }

                // Event from network or timer
                Some(event) = session_event_rx.recv() => {
                    session.handle_event(event).await?;
                }

                // Retransmit timer
                _ = retransmit_interval.tick() => {
                    session.handle_event(SessionEvent::Retransmit).await?;
                }

                // Idle check
                _ = idle_check.tick() => {
                    if session.last_activity.elapsed() > Duration::from_secs(60) {
                        session.handle_event(SessionEvent::IdleTimeout).await?;
                        break;
                    }
                }

                else => break,
            }
        }

        // Send close on exit
        let _ = session.udp_packet_pair_tx.send(UdpPacketPair::new(
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
                self.pending_out_data.extend_from_slice(&data);
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
                if pos == self.in_pos {
                    let unescaped = unescape_data(&escaped_data);
                    let bytes = unescaped.as_bytes();
                    self.in_buffer.extend_from_slice(bytes);

                    self.in_pos += bytes.len() as u64;
                    debug!("update in_pos to: {}", self.in_pos);
                    self.send_ack(self.in_pos).await;
                    // Note: we don't notify reader here — LrcpStream polls recv_buffer via channel
                } else {
                    // Request retransmission by re-acking current position
                    self.send_ack(self.in_pos).await;
                }
            }

            SessionEvent::Ack { length } => {
                if length <= self.acked_out_pos {
                    // Duplicate ACK — ignore
                } else if length > self.out_pos {
                    // Misbehaving client
                    let _x = self.send_close().await;
                    return Err("client acked more than sent".into());
                } else {
                    self.acked_out_pos = length;
                    // Truncate pending_data: everything before acked_pos is confirmed
                    let confirmed_bytes = (self.out_pos - self.acked_out_pos) as usize;
                    if confirmed_bytes < self.pending_out_data.len() {
                        self.pending_out_data
                            .drain(..(self.pending_out_data.len() - confirmed_bytes));
                    } else {
                        self.pending_out_data.clear();
                    }
                }
            }

            SessionEvent::Retransmit => {
                if !self.pending_out_data.is_empty() {
                    self.send_pending_data().await;
                }
            }

            SessionEvent::Close => {
                self.send_close().await;
            }

            SessionEvent::IdleTimeout => {
                return Err("idle timeout".into());
            }
        }

        Ok(())
    }

    async fn send_ack(&self, pos: u64) {
        let ack = format!("/ack/{}/{}", self.session_id, pos);
        let _ = self
            .udp_packet_pair_tx
            .send(UdpPacketPair::new(self.peer, ack));
    }

    async fn send_close(&self) {
        let close = format!("/close/{}/", self.session_id);
        debug!("->> {}", close);
        let _ = self
            .udp_packet_pair_tx
            .send(UdpPacketPair::new(self.peer, close));
    }

    async fn send_pending_data(&mut self) {
        if self.pending_out_data.is_empty() {
            return;
        }

        // Chunk to fit under 1000 bytes
        let data = std::str::from_utf8(&self.pending_out_data).unwrap_or_default();
        let escaped = escape_data(data);
        let message = format!("/data/{}/{}/{}/", self.session_id, self.out_pos, escaped);
        if message.len() < 1000 {
            let _ = self
                .udp_packet_pair_tx
                .send(UdpPacketPair::new(self.peer, message.clone()));
        } else {
            // TODO: chunking logic (split data into multiple /data/ messages)
            eprintln!("TODO: chunk large message");
        }
    }
}
