use super::protocol::*;
use crate::{Error, Result};
use bytes::Bytes;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tokio::time::{Interval, interval};

#[allow(unused)]
use tracing::{debug, error, info};

const MAX_DATA_LENGTH: usize = 3000;
pub const RETRANSMIT_MILLIS: usize = 3000;
const IDLE_TIMEOUT_SECOND: usize = 60;

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
pub enum SessionEvent {
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
    Ack {
        length: u64,
    },
    RepeatedConnect,
    /// From network: close
    Close {
        reason: String,
    },
    /// Retransmit timer fired
    RetransmitPendingData,
    CheckSessionExpiry,
}

/// Manage the state of a single logical connection
pub struct Session {
    session_id: u64,
    peer: std::net::SocketAddr,
    udp_packet_pair_tx: mpsc::UnboundedSender<UdpMessage>,
    session_event_tx: mpsc::UnboundedSender<SessionEvent>,
    lrcp_message_tx: mpsc::UnboundedSender<(LrcpMessage, SocketAddr)>,
    // Incoming stream
    // The next byte position the server expects to receive.
    // All bytes [0, in_pos] has been received.
    in_position: u64,

    // Outgoing stream
    // next byte offset to send (or total bytes sent so far)
    out_position: u64,
    // how many bytes the client has acknowledged
    acked_out_position: u64,

    pending_out_payload: Vec<u8>,

    last_activity: Instant,
    // ✅ New: channel to send received data to the application
    // It is used to send received data upto the application layer
    bytes_tx: mpsc::UnboundedSender<Bytes>,
    retransmit_handle: Option<AbortHandle>,
    timeout_interval: Interval,
}

#[derive(Debug)]
pub struct UdpMessage {
    pub target: std::net::SocketAddr,
    pub payload: Vec<u8>,
}

impl fmt::Display for UdpMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = std::str::from_utf8(&self.payload).unwrap();
        let output = format!("UdpPacketPair -- target: {}, payload: {}", self.target, s);
        write!(f, "{}", output)
    }
}

impl UdpMessage {
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
        peer: SocketAddr,
        udp_packet_pair_tx: mpsc::UnboundedSender<UdpMessage>,
        mut session_cmd_rx: mpsc::UnboundedReceiver<SessionCommand>,
        session_event_tx: mpsc::UnboundedSender<SessionEvent>,
        mut session_event_rx: mpsc::UnboundedReceiver<SessionEvent>,
        bytes_tx: mpsc::UnboundedSender<Bytes>,
        lrcp_message_tx: mpsc::UnboundedSender<(LrcpMessage, SocketAddr)>,
    ) -> Result<()> {
        let mut session = Self {
            session_id,
            peer,
            udp_packet_pair_tx,
            session_event_tx,
            in_position: 0,
            out_position: 0,
            acked_out_position: 0,
            pending_out_payload: Vec::new(),
            last_activity: Instant::now(),
            bytes_tx,
            retransmit_handle: None,
            timeout_interval: interval(Duration::from_secs(IDLE_TIMEOUT_SECOND as u64)),
            lrcp_message_tx,
        };

        loop {
            tokio::select! {
                // Command from LrcpStream (app)
                Some(cmd) = session_cmd_rx.recv() => {
                    session.handle_command(cmd).await?;
                }

                // Event from network or timer
                Some(event) = session_event_rx.recv() => {
                    let _ = session.handle_event(event).await?;
                }
                // Idle check
                _ = session.timeout_interval.tick() => {
                    session.handle_event(SessionEvent::CheckSessionExpiry).await?;
                }
                else => break,
            }
        }

        Ok(())
    }

    fn handle_close(&mut self) {
        // Send close on exit
        if let Some(handle) = self.retransmit_handle.take() {
            handle.abort();
        }
        let _ = self.udp_packet_pair_tx.send(UdpMessage::new(
            self.peer,
            format!("/close/{}/", self.session_id),
        ));

        let _ = self.lrcp_message_tx.send((
            LrcpMessage::SessionTerminate {
                session_id: self.session_id,
            },
            self.peer,
        ));
    }

    fn reset_session_expriry_timer(&mut self) {
        // debug!("== reset session {} exprity ==", self.session_id);
        self.timeout_interval = interval(Duration::from_secs(IDLE_TIMEOUT_SECOND as u64));
        self.last_activity = Instant::now();
    }

    /// Handle event from TcpStream application layer
    async fn handle_command(&mut self, cmd: SessionCommand) -> Result<()> {
        match cmd {
            SessionCommand::Write { data } => {
                self.pending_out_payload.extend_from_slice(&data);
                let _ = self.send_data(data).await?;
            }
            SessionCommand::Shutdown => {
                // Graceful shutdown
            }
        }
        Ok(())
    }

    /// Handle event from UDP socket, protocol logic mainly happened here.
    async fn handle_event(&mut self, event: SessionEvent) -> Result<()> {
        match event {
            SessionEvent::Close { reason } => {
                self.handle_close();
                return Err(Error::Other(format!(
                    "session {} close because {}",
                    self.session_id, reason
                )));
            }
            SessionEvent::CheckSessionExpiry => {
                // debug!("== check session: {} idle ==", self.session_id);
                if self.last_activity.elapsed() > Duration::from_secs(IDLE_TIMEOUT_SECOND as u64) {
                    self.handle_close();

                    return Err(Error::Other(format!(
                        "client is idle more than: {} seconds, close it",
                        IDLE_TIMEOUT_SECOND
                    )));
                }
            }
            SessionEvent::RepeatedConnect => {
                // let _ = self.reset_session_expriry_timer();
            }
            SessionEvent::Data { pos, escaped_data } => {
                let _ = self.reset_session_expriry_timer();

                // It means the next byte position the server expects is correct
                if pos == self.in_position {
                    let unescaped = unescape_data(&escaped_data);
                    let bytes = Bytes::from(unescaped.into_bytes());
                    let byte_len = bytes.len();

                    self.in_position += byte_len as u64;
                    self.send_ack(self.in_position).await;

                    // Send to application layer
                    let _x = self.bytes_tx.send(bytes);
                } else {
                    // Request retransmission by re-acking current position
                    self.send_ack(self.in_position).await;
                }
            }

            SessionEvent::Ack { length } => {
                // let _ = self.reset_session_expriry_timer();

                // 1. Duplicate or stale ACK: ignore
                if length <= self.acked_out_position {
                    // Spec: "If the LENGTH value is not larger than the largest... do nothing"
                    return Ok(());
                }

                // 2. Invalid ACK: client claims to have received more than we've sent
                if length > self.out_position {
                    // Spec: "If the LENGTH value is larger than the total amount... close the session"

                    self.handle_close();
                    return Err(Error::Other("client acked more bytes than sent".into()));
                }

                // 3. Valid new ACK: update state and trim send buffer
                if length < (self.acked_out_position + self.pending_out_payload.len() as u64) {
                    let transmitted_bytes = length - self.acked_out_position;

                    let _ = self.pending_out_payload.drain(..transmitted_bytes as usize);

                    let payload = format!(
                        "/data/{}/{}/{}/",
                        self.session_id,
                        self.acked_out_position + transmitted_bytes,
                        escape_data(std::str::from_utf8(&self.pending_out_payload).unwrap()),
                    );

                    let _ = self
                        .udp_packet_pair_tx
                        .send(UdpMessage::new(self.peer, payload));

                    self.acked_out_position = length;

                    return Ok(());
                }

                if length == self.out_position {
                    self.acked_out_position = length;
                    self.pending_out_payload.clear();
                    return Ok(());
                }

                return Err(Error::Other("should not reach this".into()));
            }

            SessionEvent::RetransmitPendingData => {
                self.out_position = self.out_position - self.pending_out_payload.len() as u64;
                let _x = self.send_data(self.pending_out_payload.clone()).await;
            }
        }
        Ok(())
    }

    fn schedule_retransmit(&mut self) {
        // Cancel any previous retransmit task
        if let Some(handle) = self.retransmit_handle.take() {
            handle.abort();
        }

        let tx = self.session_event_tx.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(RETRANSMIT_MILLIS as u64)).await;
            let _ = tx.send(SessionEvent::RetransmitPendingData);
        });

        self.retransmit_handle = Some(handle.abort_handle());
    }

    async fn send_ack(&self, pos: u64) {
        let ack = format!("/ack/{}/{}/", self.session_id, pos);
        let _ = self
            .udp_packet_pair_tx
            .send(UdpMessage::new(self.peer, ack));
    }

    async fn send_data(&mut self, data: Vec<u8>) -> Result<()> {
        for each in produce_chunks(data.clone(), MAX_DATA_LENGTH) {
            let each_str = match std::str::from_utf8(&each) {
                Ok(s) => s,
                Err(_) => {
                    return Err(Error::Other("Non-UTF8 data in send_data".into()));
                }
            };

            let _ = self.udp_packet_pair_tx.send(UdpMessage::new(
                self.peer,
                format!(
                    "/data/{}/{}/{}/",
                    self.session_id,
                    self.out_position,
                    escape_data(each_str)
                ),
            ));
            self.out_position = self.out_position + each.len() as u64;
        }

        self.schedule_retransmit();

        Ok(())
    }
}

fn produce_chunks(data: Vec<u8>, chunk_size: usize) -> Vec<Vec<u8>> {
    if chunk_size == 0 {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut remaining = data;
    while !remaining.is_empty() {
        let take = remaining.len().min(chunk_size);
        let (chunk, rest) = remaining.split_at(take);
        chunks.push(chunk.to_vec());
        remaining = rest.to_vec();
    }
    chunks
}
