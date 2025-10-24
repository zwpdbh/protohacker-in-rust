use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::protocol::*;
use super::session::*;
use super::stream::LrcpStream;

#[derive(Clone)]
pub struct LrcpListener {
    sessions: HashMap<u64, mpsc::UnboundedSender<SessionEvent>>,
    udp_tx: mpsc::UnboundedSender<UdpPacket>,
    accept_tx: mpsc::UnboundedSender<(LrcpStream, std::net::SocketAddr)>,
}

impl LrcpListener {
    pub async fn bind(
        addr: &str,
    ) -> Result<
        (
            Self,
            mpsc::UnboundedReceiver<(LrcpStream, std::net::SocketAddr)>,
        ),
        Box<dyn std::error::Error>,
    > {
        let socket = UdpSocket::bind(addr).await?;
        let (udp_tx, mut udp_rx) = mpsc::unbounded_channel::<UdpPacket>();
        let (accept_tx, accept_rx) = mpsc::unbounded_channel();

        // Spawn UDP send loop
        let socket2 = socket.try_clone()?;
        tokio::spawn(async move {
            while let Some(pkt) = udp_rx.recv().await {
                let _ = socket2.send_to(&pkt.payload, pkt.target).await;
            }
        });

        let listener = Self {
            sessions: HashMap::new(),
            udp_tx,
            accept_tx,
        };

        // Spawn UDP receive loop
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        if let Some(pkt) = parse_packet(&buf[..len]) {
                            listener.clone().handle_packet(src, pkt).await;
                        }
                    }
                    Err(e) => eprintln!("UDP recv error: {}", e),
                }
            }
        });

        Ok((listener, accept_rx))
    }

    async fn handle_packet(&mut self, src: std::net::SocketAddr, pkt: LrcpPacket) {
        match pkt.kind {
            LrcpPacketKind::Connect => {
                // Always send ACK, even for duplicate connects
                let ack = format!("/ack/{}/0/", pkt.session_id);
                let _ = self.udp_tx.send(UdpPacket::new(src, ack));

                // Only spawn a new session if we don't already have one
                if !self.sessions.contains_key(&pkt.session_id) {
                    // Create channels for session communication
                    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel(); // App → Session (writes)
                    let (event_tx, event_rx) = mpsc::unbounded_channel(); // Network → Session (events)
                    let (read_tx, read_rx) = mpsc::unbounded_channel(); // Session → App (reads)

                    // Create the stream that the application layer will use
                    let stream = LrcpStream::new(cmd_tx, read_rx);

                    // Spawn the session actor
                    let udp_tx = self.udp_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) =
                            Session::spawn(pkt.session_id, src, udp_tx, cmd_rx, event_rx, read_tx)
                                .await
                        {
                            eprintln!("Session {} error: {}", pkt.session_id, e);
                        }
                    });

                    // Store the event sender so future packets can be routed
                    self.sessions.insert(pkt.session_id, event_tx);

                    // Offer the stream to the acceptor (your `run` function)
                    let _ = self.accept_tx.send((stream, src));
                }
            }

            // Handle other packet types
            _ => {
                if let Some(event_tx) = self.sessions.get(&pkt.session_id) {
                    match pkt.kind {
                        LrcpPacketKind::Data { pos, escaped_data } => {
                            let _ = event_tx.send(SessionEvent::Data { pos, escaped_data });
                        }
                        LrcpPacketKind::Ack { length } => {
                            let _ = event_tx.send(SessionEvent::Ack { length });
                        }
                        LrcpPacketKind::Close => {
                            let _ = event_tx.send(SessionEvent::Close);
                            self.sessions.remove(&pkt.session_id);
                        }
                        // Connect is already handled above
                        LrcpPacketKind::Connect => unreachable!(),
                    }
                } else {
                    // Unknown session — send /close/
                    let close = format!("/close/{}/", pkt.session_id);
                    let _ = self.udp_tx.send(UdpPacket::new(src, close));
                }
            }
        }
    }
}
