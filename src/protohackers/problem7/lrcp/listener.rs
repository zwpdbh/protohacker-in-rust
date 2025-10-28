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
        let (udp_packet_pair_tx, mut udp_packet_pair_rx) =
            mpsc::unbounded_channel::<UdpPacketPair>();
        let (lrcp_packet_pair_tx, mut lrcp_packet_pair_rx) =
            mpsc::unbounded_channel::<LrcpPacketPair>();
        let (lrcp_stream_pair_tx, lrcp_stream_pair_rx) =
            mpsc::unbounded_channel::<LrcpStreamPair>();

        // UDP I/O task: handles both sending and receiving from raw UDP socket
        tokio::spawn(async move {
            let mut recv_buf = [0u8; 1024];
            loop {
                tokio::select! {
                    // Send outgoing LRCP packets
                    Some(pkt) = udp_packet_pair_rx.recv() => {
                        debug!("udp socket send out going LRCP packet: {:?}", pkt);
                        let _ = socket.send_to(&pkt.payload, pkt.target).await;
                    }

                    // Receive incoming UDP packets and create LrcpPacketPair
                    recv_result = socket.recv_from(&mut recv_buf) => {
                        match recv_result {
                            Ok((len, addr)) => {
                                if let Ok(lrcp_packet) = parse_packet(&recv_buf[..len]) {
                                    let _ = lrcp_packet_pair_tx.send(LrcpPacketPair { lrcp_packet, addr });
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
        let udp_packet_pair_tx_clone = udp_packet_pair_tx.clone();
        let lrcp_stream_pair_tx_clone = lrcp_stream_pair_tx.clone();
        tokio::spawn(async move {
            let mut sessions: HashMap<u64, mpsc::UnboundedSender<SessionEvent>> = HashMap::new();
            while let Some(lrcp_packet_pair) = lrcp_packet_pair_rx.recv().await {
                Self::route_packet(
                    &mut sessions,
                    &udp_packet_pair_tx_clone,
                    &lrcp_stream_pair_tx_clone,
                    lrcp_packet_pair,
                )
                .await;
            }
        });

        Ok(Self {
            accept_rx: lrcp_stream_pair_rx,
        })
    }

    async fn route_packet(
        sessions: &mut HashMap<u64, mpsc::UnboundedSender<SessionEvent>>,
        udp_packet_pair_tx: &mpsc::UnboundedSender<UdpPacketPair>,
        lrcp_stream_pair_tx: &mpsc::UnboundedSender<LrcpStreamPair>,
        lrcp_packet_pair: LrcpPacketPair,
    ) {
        match lrcp_packet_pair.lrcp_packet {
            LrcpPacket::Connect { session_id } => {
                debug!("connect client: {session_id}");
                // Always ACK, even for duplicates
                let ack = format!("/ack/{}/0/", session_id);
                let _ = udp_packet_pair_tx.send(UdpPacketPair::new(lrcp_packet_pair.addr, ack));

                if !sessions.contains_key(&session_id) {
                    // Create channels
                    let (session_cmd_tx, session_cmd_rx) = mpsc::unbounded_channel();
                    let (session_event_tx, session_event_rx) = mpsc::unbounded_channel();
                    let (bytes_tx, bytes_rx) = mpsc::unbounded_channel();

                    // Create stream for application
                    let lrcp_stream = LrcpStream::new(session_cmd_tx, bytes_rx);

                    // Spawn session actor
                    let udp_packet_paire_tx_clone = udp_packet_pair_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Session::spawn(
                            session_id,
                            lrcp_packet_pair.addr,
                            udp_packet_paire_tx_clone,
                            session_cmd_rx,
                            session_event_rx,
                            bytes_tx,
                        )
                        .await
                        {
                            error!("Session {} error: {}", session_id, e);
                        }
                    });

                    // Store event sender for routing future packets
                    sessions.insert(session_id, session_event_tx);

                    // Offer stream to acceptor
                    let _ = lrcp_stream_pair_tx
                        .send(LrcpStreamPair::new(lrcp_stream, lrcp_packet_pair.addr));
                } else {
                    error!("client {session_id} sent repeated connect command");
                }
            }
            LrcpPacket::Data {
                session_id,
                pos,
                escaped_data,
            } => {
                debug!(
                    "client: {session_id} receive data: {}, pos: {}",
                    escaped_data, pos
                );
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Data { pos, escaped_data });
                } else {
                    // If the session is not open: send /close/SESSION/ and stop.
                    let close = format!("/close/{}/", session_id);
                    let _ =
                        udp_packet_pair_tx.send(UdpPacketPair::new(lrcp_packet_pair.addr, close));
                }
            }
            LrcpPacket::Ack { session_id, length } => {
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Ack { length });
                }
            }
            LrcpPacket::Close { session_id } => {
                if let Some(session_event_tx) = sessions.get(&session_id) {
                    let _ = session_event_tx.send(SessionEvent::Close);
                    sessions.remove(&session_id);
                }
            }
        }
    }
}
