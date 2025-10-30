use super::protocol::*;
use bytes::Bytes;
use std::fmt;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use tokio::time::interval;
#[allow(unused)]
use tracing::{debug, error, info};

/// It is the communication channel from the application layer
/// down into the LRCP session state machine.
#[derive(Debug)]
pub enum SessionCommand {
    /// App wants to write data to the stream
    Write { data: Vec<u8> },
    /// App wants to read data (non-blocking poll)
    /// We'll use a different mechanism for AsyncRead (see LrcpStream)
    #[allow(unused)]
    Shutdown,
}

/// A session is a logical connection established with a UDP socket.
/// This event represent LRCP transport layer event for a session.
/// Once a UdpPacket is routed to a session, it becomes an LrcpEvent.
/// It also includes timer-driven events: like retransmit and idle timeout.
/// It is used to drive the session's state machine in `handle_event` from loop.
#[derive(Debug)]
pub enum LrcpEvent {
    /// From network: data packet
    Data {
        /// The stream offset of the first byte in this /data/ message
        /// the unescaped payload in this packet belongs at offset `pos` in your input stream.
        /// The receiver us it to:
        /// 1. detect missing chunks (if pos > expected)
        /// 2. discard duplicates (if pos < expected)
        /// 3. accept in-order data (if pos == expected)
        pos: u64,
        escaped_data: String,
    },
    /// Total number of contiguous bytes the receiver has successfully received,
    /// starting from byte 0.
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
    // The next byte position the server expects to receive.
    // All bytes [0, in_pos] has been received.
    in_pos: u64,

    // Outgoing stream
    // next byte offset to send (or total bytes sent so far)
    out_pos: u64,
    // how many bytes the client has acknowledged
    acked_out_pos: u64,

    pending_out_data: Vec<u8>,
    // stream offset of pending_out_data[0]
    pending_out_base: u64,

    last_activity: Instant,
    // ✅ New: channel to send received data to the application
    // It is used to send received data upto the application layer
    bytes_tx: mpsc::UnboundedSender<Bytes>,
}

#[derive(Debug)]
pub struct UdpPacketPair {
    pub target: std::net::SocketAddr,
    pub payload: Vec<u8>,
}

impl fmt::Display for UdpPacketPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = std::str::from_utf8(&self.payload).unwrap();
        let output = format!("UdpPacketPair -- target: {}, payload: {}", self.target, s);
        write!(f, "{}", output)
    }
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
        mut session_event_rx: mpsc::UnboundedReceiver<LrcpEvent>,
        bytes_tx: mpsc::UnboundedSender<Bytes>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut session = Self {
            session_id,
            peer,
            udp_packet_pair_tx,
            in_pos: 0,
            out_pos: 0,
            acked_out_pos: 0,
            pending_out_base: 0,
            pending_out_data: Vec::new(),
            last_activity: Instant::now(),
            bytes_tx,
        };

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
                    session.handle_event(LrcpEvent::Retransmit).await?;
                }

                // Idle check
                _ = idle_check.tick() => {
                    if session.last_activity.elapsed() > Duration::from_secs(60) {
                        session.handle_event(LrcpEvent::IdleTimeout).await?;
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
            SessionCommand::Write { data } => {
                self.pending_out_data.extend_from_slice(&data);
                self.out_pos += data.len() as u64;
                self.send_pending_data().await;
            }
            SessionCommand::Shutdown => {
                // Graceful shutdown
            }
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: LrcpEvent) -> Result<(), Box<dyn std::error::Error>> {
        self.last_activity = Instant::now();

        match event {
            LrcpEvent::Data { pos, escaped_data } => {
                // It means the next byte position the server expects is correct
                if pos == self.in_pos {
                    let unescaped = unescape_data(&escaped_data);
                    let bytes = Bytes::from(unescaped.into_bytes());
                    let byte_len = bytes.len();

                    // ✅ Send to application layer
                    let _x = self.bytes_tx.send(bytes);

                    self.in_pos += byte_len as u64;
                    self.send_ack(self.in_pos).await;
                } else {
                    // Request retransmission by re-acking current position
                    self.send_ack(self.in_pos).await;
                }
            }

            LrcpEvent::Ack { length } => {
                // 1. Duplicate or stale ACK: ignore
                if length <= self.acked_out_pos {
                    // Spec: "If the LENGTH value is not larger than the largest... do nothing"
                    return Ok(());
                }

                // 2. Invalid ACK: client claims to have received more than we've sent
                if length > self.out_pos {
                    // Spec: "If the LENGTH value is larger than the total amount... close the session"
                    self.send_close().await;
                    return Err("client acked more bytes than sent".into());
                }

                // 3. Valid new ACK: update state and trim send buffer
                debug_assert!(self.acked_out_pos < length && length <= self.out_pos);

                let newly_acked = length - self.acked_out_pos;
                self.acked_out_pos = length;

                // Remove the newly acknowledged prefix from pending_out_data
                if newly_acked as usize >= self.pending_out_data.len() {
                    self.pending_out_data.clear();
                    self.pending_out_base = length;
                } else {
                    let _ = self.pending_out_data.drain(..newly_acked as usize);
                    self.pending_out_base = length;
                }
            }

            LrcpEvent::Retransmit => {
                if !self.pending_out_data.is_empty() {
                    self.send_pending_data().await;
                }
            }

            LrcpEvent::Close => {
                self.send_close().await;
            }

            LrcpEvent::IdleTimeout => {
                return Err("idle timeout".into());
            }
        }

        Ok(())
    }

    async fn send_ack(&self, pos: u64) {
        let ack = format!("/ack/{}/{}/", self.session_id, pos);
        let _ = self
            .udp_packet_pair_tx
            .send(UdpPacketPair::new(self.peer, ack));
    }

    async fn send_close(&self) {
        let close = format!("/close/{}/", self.session_id);
        let _ = self
            .udp_packet_pair_tx
            .send(UdpPacketPair::new(self.peer, close));
    }

    async fn send_pending_data(&mut self) {
        if self.pending_out_data.is_empty() {
            return;
        }

        // The stream position of the first byte in pending_out_data
        let pos = self.pending_out_base;

        let data_str = match std::str::from_utf8(&self.pending_out_data) {
            Ok(s) => s,
            Err(_) => {
                error!("Non-utf8 data in sending_pending_data");
                return;
            }
        };

        let escaped = escape_data(data_str);
        let message = format!("/data/{}/{}/{}/", self.session_id, pos, escaped);

        if message.len() < 1000 {
            let _ = self
                .udp_packet_pair_tx
                .send(UdpPacketPair::new(self.peer, message));
        } else {
            // 🚧 For now, just log — but ideally, chunk here
            todo!(
                "Message too large ({} bytes). Implement chunking!",
                message.len()
            );
            // TODO: split `self.pending_out_data` into chunks that fit after escaping
        }
    }
}
