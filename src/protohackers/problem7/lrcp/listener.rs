use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::protocol::*;
use super::session::*;
use super::stream::*;
use crate::{Error, Result};
use std::net::SocketAddr;
use tracing::debug;
use tracing::error;
#[allow(unused)]
use tracing::instrument;

pub struct LrcpListener {
    // pub udp_tx: mpsc::UnboundedSender<UdpPacket>,
    // pub accept_tx: mpsc::UnboundedSender<(LrcpStream, SocketAddr)>,
    pub accept_rx: mpsc::UnboundedReceiver<LrcpStreamPair>,
}

impl LrcpListener {
    pub async fn accept(&mut self) -> Result<(LrcpStream, SocketAddr)> {
        let lrcp_accept_result = self
            .accept_rx
            .recv()
            .await
            .ok_or_else(|| Error::Other("failed to init LrcpListener".into()))?;
        Ok((lrcp_accept_result.stream, lrcp_accept_result.addr))
    }

    pub async fn bind(addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        let (udp_message_tx, mut udp_message_rx) = mpsc::unbounded_channel::<UdpMessage>();
        let (lrcp_message_tx, mut lrcp_message_rx) =
            mpsc::unbounded_channel::<(LrcpMessage, SocketAddr)>();
        let (lrcp_stream_tx, lrcp_stream_rx) = mpsc::unbounded_channel::<LrcpStreamPair>();
        let lrcp_message_tx_clone = lrcp_message_tx.clone();

        // UDP I/O task: handles both sending and receiving from raw UDP socket
        tokio::spawn(async move {
            let mut recv_buf = [0u8; 1024];
            loop {
                tokio::select! {
                    // Send outgoing LRCP packets
                    Some(pkt) = udp_message_rx.recv() => {
                        debug!("->> send udp_packet: {}", pkt);
                        let _ = socket.send_to(&pkt.payload, pkt.target).await;
                    }

                    // Receive incoming UDP packets and create LrcpPacketPair
                    recv_result = socket.recv_from(&mut recv_buf) => {
                        match recv_result {
                            Ok((len, addr)) => {
                                if let Ok(lrcp_message) = parse_packet(&recv_buf[..len]) {
                                    debug!("<<- received lrcp_packet: {:?}", lrcp_message);
                                    let _ = lrcp_message_tx.send((lrcp_message, addr));
                                }
                            }
                            Err(e) => {
                                error!("UDP recv error: {}", e);
                            }
                        }
                    }
                }
            }
        });

        // Session router task: owns the session map and routes packets
        // Routes parsed protocol message to per-session actors
        let udp_messge_tx_clone = udp_message_tx.clone();
        let lrcp_stream_tx_clone = lrcp_stream_tx.clone();
        tokio::spawn(async move {
            let mut sessions: HashMap<u64, mpsc::UnboundedSender<SessionEvent>> = HashMap::new();
            while let Some((lrcp_message, addr)) = lrcp_message_rx.recv().await {
                Self::route_lrcp_message(
                    &mut sessions,
                    addr,
                    &udp_messge_tx_clone,
                    &lrcp_stream_tx_clone,
                    lrcp_message,
                    &lrcp_message_tx_clone,
                )
                .await;
            }
        });

        Ok(Self {
            accept_rx: lrcp_stream_rx,
        })
    }

    // #[instrument(skip(sessions, udp_packet_pair_tx, lrcp_stream_pair_tx))]
    async fn route_lrcp_message(
        sessions: &mut HashMap<u64, mpsc::UnboundedSender<SessionEvent>>,
        addr: SocketAddr,
        udp_messge_tx: &mpsc::UnboundedSender<UdpMessage>,
        lrcp_stream_tx: &mpsc::UnboundedSender<LrcpStreamPair>,
        lrcp_message: LrcpMessage,
        lrcp_message_tx: &mpsc::UnboundedSender<(LrcpMessage, SocketAddr)>,
    ) {
        match lrcp_message {
            LrcpMessage::Connect { session_id } => {
                // Always ACK, even for duplicates
                let ack = format!("/ack/{}/0/", session_id);
                let _ = udp_messge_tx.send(UdpMessage::new(addr, ack));

                match sessions.get(&session_id) {
                    Some(session) => {
                        let _ = session.send(SessionEvent::RepeatedConnect);
                    }
                    None => {
                        // Create channels
                        let (session_cmd_tx, session_cmd_rx) = mpsc::unbounded_channel();
                        let (session_event_tx, session_event_rx) = mpsc::unbounded_channel();
                        let (bytes_tx, bytes_rx) = mpsc::unbounded_channel();

                        // Create stream for application
                        let lrcp_stream = LrcpStream::new(session_cmd_tx, bytes_rx);

                        // Spawn session actor
                        let udp_packet_paire_tx_clone = udp_messge_tx.clone();
                        let session_event_tx_clone = session_event_tx.clone();
                        let lrcp_message_tx_clone = lrcp_message_tx.clone();

                        tokio::spawn(async move {
                            if let Err(e) = Session::spawn(
                                session_id,
                                addr,
                                udp_packet_paire_tx_clone,
                                session_cmd_rx,
                                session_event_tx_clone,
                                session_event_rx,
                                bytes_tx,
                                lrcp_message_tx_clone,
                            )
                            .await
                            {
                                error!("== session {} error: {}", session_id, e);
                            }
                        });

                        // Store event sender for routing future packets
                        sessions.insert(session_id, session_event_tx);

                        // Offer stream to acceptor
                        let _ = lrcp_stream_tx.send(LrcpStreamPair::new(lrcp_stream, addr));
                    }
                }
            }
            LrcpMessage::Data {
                session_id,
                pos,
                escaped_data,
            } => {
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Data { pos, escaped_data });
                } else {
                    // If the session is not open: send /close/SESSION/ and stop.
                    let close = format!("/close/{}/", session_id);
                    let _ = udp_messge_tx.send(UdpMessage::new(addr, close));
                }
            }
            LrcpMessage::Ack { session_id, length } => {
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Ack { length });
                }
            }
            LrcpMessage::ClientClose { session_id } => {
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Close {
                        reason: "client close connection".to_string(),
                    });
                } else {
                    let close = format!("/close/{}/", session_id);
                    let _ = udp_messge_tx.send(UdpMessage::new(addr, close));
                }
            }
            LrcpMessage::SessionTerminate { session_id } => {
                debug!("session {} terminated", session_id);
                let _ = sessions.remove(&session_id);
            }
        }
    }
}
