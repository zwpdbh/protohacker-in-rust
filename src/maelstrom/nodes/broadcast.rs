use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::error;
pub struct BroadcastNode {
    base: BaseNode,
    id_gen: IdGenerator,
    topology: HashMap<String, Vec<String>>,
    messages: HashSet<usize>,
    neighbors: Vec<String>,
    gossip_records: HashMap<String, HashSet<usize>>,
    myself_tx: Option<mpsc::UnboundedSender<NodeEvent>>,
}

impl BroadcastNode {
    pub fn new() -> Self {
        Self {
            base: BaseNode::new(),
            id_gen: IdGenerator::new(),
            topology: HashMap::new(),
            messages: HashSet::new(),
            neighbors: Vec::new(),
            gossip_records: HashMap::new(),
            myself_tx: None,
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
                self.neighbors = self.topology.remove(&self.base.node_id).ok_or_else(|| {
                    Error::Other(format!(
                        "node {} has no associated neighbours",
                        self.base.node_id
                    ))
                })?;

                self.base.send_msg_to_output(reply).await?;
            }

            Payload::Broadcast { message } => {
                self.messages.insert(*message);
                self.udpate_gossiped_message(&msg.src, *message);

                let reply = msg.into_reply(None, Payload::BroadcastOk);
                self.base.send_msg_to_output(reply).await?;

                let myself_tx_clone = self.myself_tx.clone().unwrap();
                let _x = myself_tx_clone.send(NodeEvent::Internal(NodeMessage::Gossip));
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
            // receive gossip message sent by other node
            Payload::Gossip { messages } => {
                for each in messages {
                    self.messages.insert(*each);
                    self.udpate_gossiped_message(&msg.src, *each);
                }
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
        self.myself_tx = Some(tx.clone());

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
        let mut interval = tokio::time::interval(Duration::from_millis(500));

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

    fn gossiped(&self, node: &str, message: usize) -> bool {
        match self.gossip_records.get(node) {
            Some(gossiped_messages) => gossiped_messages.contains(&message),
            None => false,
        }
    }

    fn udpate_gossiped_message(&mut self, node: &str, message: usize) {
        self.gossip_records
            .entry(node.to_string())
            .or_insert_with(|| HashSet::new())
            .insert(message);
    }

    async fn handle_node_message(&mut self, msg: NodeMessage) -> Result<()> {
        match msg {
            NodeMessage::Gossip => {
                use rand::prelude::IndexedRandom;

                let nodes = self.neighbors.clone();
                let n = (nodes.len() / 2) + 1;

                let selection: Vec<String> = nodes
                    .choose_multiple(&mut rand::rng(), n)
                    .cloned()
                    .collect();

                for each_node in selection.clone() {
                    let messages_not_gossiped: Vec<usize> = self
                        .messages
                        .iter()
                        .filter(|each_message| !self.gossiped(&each_node, **each_message))
                        .map(|each| *each)
                        // .take(10)
                        .collect();

                    let _ = self
                        .send_gossip_message(&each_node, &messages_not_gossiped)
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn send_gossip_message(
        &mut self,
        target_node: &str,
        gossip_messages: &Vec<usize>,
    ) -> Result<()> {
        let msg = Message {
            src: self.base.node_id.clone(),
            dst: target_node.to_string(),
            body: MessageBody {
                msg_id: None,
                in_reply_to: None,
                payload: Payload::Gossip {
                    messages: gossip_messages.clone(),
                },
            },
        };
        let _ = self.base.send_msg_to_output(msg).await?;

        for each_message in gossip_messages {
            self.udpate_gossiped_message(&target_node, *each_message);
        }

        Ok(())
    }
}
