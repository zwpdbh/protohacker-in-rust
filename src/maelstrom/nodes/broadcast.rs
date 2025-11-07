use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::error;

pub struct BroadcastNode {
    base: BaseNode,
    id_gen: IdGenerator,
    topology: HashMap<String, Vec<String>>,
    messages: Vec<usize>,
}

impl BroadcastNode {
    pub fn new() -> Self {
        Self {
            base: BaseNode::new(),
            id_gen: IdGenerator::new(),
            topology: HashMap::new(),
            messages: Vec::new(),
        }
    }
}

impl Node for BroadcastNode {
    async fn handle_message(&mut self, msg: Message) -> Result<()> {
        match &msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);

                let reply = msg.into_reply(Some(self.base.next_msg_id()), Payload::InitOk);

                self.base.send_msg_to_output(reply).await?;
            }
            Payload::Generate => {
                let unique_id = self.id_gen.next_id(&self.base.node_id);
                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::GenerateOk { id: unique_id },
                );

                self.base.send_msg_to_output(reply).await?;
            }
            Payload::Topology { topology } => {
                self.topology = topology.clone();
                let reply = msg.into_reply(None, Payload::TopologyOk);

                self.base.send_msg_to_output(reply).await?;
            }

            Payload::Broadcast { message } => {
                self.messages.push(*message);
                let reply = msg.into_reply(None, Payload::BroadcastOk);

                self.base.send_msg_to_output(reply).await?;
            }
            Payload::Read => {
                let reply = msg.into_reply(
                    None,
                    Payload::ReadOk {
                        messages: self.messages.clone(),
                    },
                );
                self.base.send_msg_to_output(reply).await?;
            }
            Payload::TopologyOk | Payload::BroadcastOk | Payload::ReadOk { .. } => {
                error!("ignore: {:?}", msg)
            }
            other => {
                let error_msg = format!("{:?} should not happen", other);
                error!(error_msg);
                return Err(Error::Other(error_msg));
            }
        }
        Ok(())
    }

    /// Patterns for building robust multi-event systems in Rust with Tokio:
    /// 1. Unified event enum - NodeEvent in your case with External(Message) and Internal(NodeMessage) variants
    /// 2. Centralized event bus - The mpsc::unbounded_channel that collects all events from various sources
    /// 3. Distributed event generation - Each task gets a sender to emit events to the central bus
    /// 4. Event processing loop - Main event loop using tokio::select! to handle events from the bus
    /// 5. Coordinated cancellation - Broadcast channel for clean shutdown signals
    async fn run(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::unbounded_channel::<NodeEvent>();
        let tx_clone = tx.clone();

        // Create a broadcast channel for cancellation signals
        let (cancel_tx, _) = tokio::sync::broadcast::channel::<()>(1);

        // Spawn tasks with their own cancellation receivers
        let mut stdin_task = tokio::spawn(BroadcastNode::generate_events_from_stdin_with_cancel(
            tx,
            cancel_tx.subscribe(),
        ));
        let mut ticker_task =
            tokio::spawn(BroadcastNode::generate_events_from_time_ticker_with_cancel(
                tx_clone,
                cancel_tx.subscribe(),
            ));

        loop {
            tokio::select! {
                // Handle events from channels
                Some(event) = rx.recv() => {
                    match event {
                        NodeEvent::External(msg) => {
                            if let Err(e) = self.handle_message(msg).await {
                                error!("Error handling external message: {}", e);
                            }
                        }
                        NodeEvent::Internal(msg) => {
                            if let Err(e) = self.handle_node_message(msg).await {
                                error!("Error handling internal message: {}",e);
                            }
                        }
                    }
                }
                // awaiting the JoinHandle<T> returned by tokio::spawn(...)
                _result = &mut stdin_task => {
                    let _ = cancel_tx.send(()); // Cancel everything
                    break;
                }
                _result = &mut ticker_task => {
                    let _ = cancel_tx.send(()); // Cancel everything
                    break;
                }
            }
        }
        Ok(())
    }
}

impl BroadcastNode {
    async fn generate_events_from_stdin_with_cancel(
        tx: mpsc::UnboundedSender<NodeEvent>,
        mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        loop {
            tokio::select! {
                line_result = reader.next_line() => {
                    let line = match line_result? {
                        Some(l) => l,
                        None => break, // EOF reached
                    };
                    if line.trim().is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<Message>(&line) {
                        Ok(msg) => {
                            if tx.send(NodeEvent::External(msg)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse JSON: {}", e);
                            continue;
                        }
                    }
                }
                _ = cancel_rx.recv() => {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn generate_events_from_time_ticker_with_cancel(
        tx: mpsc::UnboundedSender<NodeEvent>,
        mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(300));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if tx.send(NodeEvent::Internal(NodeMessage::Gossip)).is_err() {
                        break;
                    }
                }
                _ = cancel_rx.recv() => {
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_node_message(&mut self, msg: NodeMessage) -> Result<()> {
        match msg {
            NodeMessage::Gossip => {
                // Handle gossip message - for now just a placeholder
                // In a real implementation, you might broadcast messages to neighbors
                // tracing::debug!("Handling gossip message");
            }
        }
        Ok(())
    }
}
