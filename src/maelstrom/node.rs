use super::protocol::Message;
use crate::{Error, Result};
use std::io::StdoutLock;
use std::io::Write;

pub trait Node {
    /// Handle a message and optionally send a reply.
    /// Return `Ok(true)` if the message was handled, `Ok(false)` to fall back to default handling.
    fn handle_message(&mut self, msg: Message, output: &mut StdoutLock) -> Result<()>;

    /// Send a reply message (shared logic)
    fn send_reply(&mut self, reply: Message, output: &mut StdoutLock) -> Result<()> {
        serde_json::to_writer(&mut *output, &reply)
            .map_err(|e| Error::Other(format!("failed to serialize reply: {}", e)))?;
        output.write_all(b"\n")?;
        Ok(())
    }
}

/// It is concrete struct that encapsulates shared
/// state and behavior common to all Maelstrom node implementations.
/// Other specific node reuse it via composition, delegate common feature to it.
#[derive(Debug)]
pub struct BaseNode {
    pub node_id: String,
    pub node_ids: Vec<String>,
    msg_counter: usize,
}

impl BaseNode {
    pub fn new() -> Self {
        Self {
            node_id: String::new(),
            node_ids: Vec::new(),
            msg_counter: 1, // start at 1 for msg_id
        }
    }

    pub fn next_msg_id(&mut self) -> usize {
        let id = self.msg_counter;
        self.msg_counter += 1;
        id
    }

    pub fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.node_id = node_id;
        self.node_ids = node_ids;
    }
}

/// Extract ID generation feature in a shared abstraction.
/// Other node will compose it and delegate id generation to it
#[derive(Debug)]
pub struct IdGenerator {
    counter: u64,
}

impl IdGenerator {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// generate a unique id based on current node_id
    pub fn next_id(&mut self, node_id: &str) -> String {
        self.counter += 1;
        format!("{}-{}", node_id, self.counter)
    }
}
