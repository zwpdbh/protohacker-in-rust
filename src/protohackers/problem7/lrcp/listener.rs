use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::protocol::*;
use super::session::*;
use super::stream::LrcpStream;
use std::net::SocketAddr;

pub struct LrcpListener {
    // pub udp_tx: mpsc::UnboundedSender<UdpPacket>,
    // pub accept_tx: mpsc::UnboundedSender<(LrcpStream, SocketAddr)>,
    pub accept_rx: mpsc::UnboundedReceiver<(LrcpStream, SocketAddr)>,
}

impl LrcpListener {
    pub async fn accept(&mut self) -> Option<(LrcpStream, SocketAddr)> {
        self.accept_rx.recv().await
    }

    pub async fn bind(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind(addr).await?;
        let (udp_tx, mut udp_rx) = mpsc::unbounded_channel::<UdpPacket>();
        let (packet_tx, mut packet_rx) = mpsc::unbounded_channel::<(SocketAddr, LrcpPacket)>();
        let (accept_tx, accept_rx) = mpsc::unbounded_channel();

        // 🌐 UDP I/O task: handles both sending and receiving
        tokio::spawn(async move {
            let mut recv_buf = [0u8; 1024];
            loop {
                tokio::select! {
                    // Send outgoing LRCP packets
                    Some(pkt) = udp_rx.recv() => {
                        let _ = socket.send_to(&pkt.payload, pkt.target).await;
                    }

                    // Receive incoming UDP packets
                    recv_result = socket.recv_from(&mut recv_buf) => {
                        match recv_result {
                            Ok((len, src)) => {
                                if let Some(pkt) = parse_packet(&recv_buf[..len]) {
                                    let _ = packet_tx.send((src, pkt));
                                }
                            }
                            Err(e) => {
                                eprintln!("UDP recv error: {}", e);
                            }
                        }
                    }
                }
            }
        });

        // 🧠 Session router task: owns the session map and routes packets
        let udp_tx_router = udp_tx.clone();
        let accept_tx_router = accept_tx.clone();
        tokio::spawn(async move {
            let mut sessions: HashMap<u64, mpsc::UnboundedSender<SessionEvent>> = HashMap::new();
            while let Some((src, pkt)) = packet_rx.recv().await {
                Self::route_packet(&mut sessions, &udp_tx_router, &accept_tx_router, src, pkt)
                    .await;
            }
        });

        Ok(Self { accept_rx })
    }

    async fn route_packet(
        sessions: &mut HashMap<u64, mpsc::UnboundedSender<SessionEvent>>,
        udp_tx: &mpsc::UnboundedSender<UdpPacket>,
        accept_tx: &mpsc::UnboundedSender<(LrcpStream, SocketAddr)>,
        src: SocketAddr,
        pkt: LrcpPacket,
    ) {
        match pkt.kind {
            LrcpPacketKind::Connect => {
                // Always ACK, even for duplicates
                let ack = format!("/ack/{}/0/", pkt.session_id);
                let _ = udp_tx.send(UdpPacket::new(src, ack));

                if !sessions.contains_key(&pkt.session_id) {
                    // Create channels
                    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
                    let (event_tx, event_rx) = mpsc::unbounded_channel();
                    let (read_tx, read_rx) = mpsc::unbounded_channel();

                    // Create stream for application
                    let stream = LrcpStream::new(cmd_tx, read_rx);

                    // Spawn session actor
                    let udp_tx_clone = udp_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Session::spawn(
                            pkt.session_id,
                            src,
                            udp_tx_clone,
                            cmd_rx,
                            event_rx,
                            read_tx,
                        )
                        .await
                        {
                            eprintln!("Session {} error: {}", pkt.session_id, e);
                        }
                    });

                    // Store event sender for routing future packets
                    sessions.insert(pkt.session_id, event_tx);

                    // Offer stream to acceptor
                    let _ = accept_tx.send((stream, src));
                }
            }

            _ => {
                if let Some(event_tx) = sessions.get(&pkt.session_id) {
                    match pkt.kind {
                        LrcpPacketKind::Data { pos, escaped_data } => {
                            let _ = event_tx.send(SessionEvent::Data { pos, escaped_data });
                        }
                        LrcpPacketKind::Ack { length } => {
                            let _ = event_tx.send(SessionEvent::Ack { length });
                        }
                        LrcpPacketKind::Close => {
                            let _ = event_tx.send(SessionEvent::Close);
                            sessions.remove(&pkt.session_id);
                        }
                        LrcpPacketKind::Connect => unreachable!(),
                    }
                } else {
                    // Unknown session — send /close/
                    let close = format!("/close/{}/", pkt.session_id);
                    let _ = udp_tx.send(UdpPacket::new(src, close));
                }
            }
        }
    }
}
