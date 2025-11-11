use super::protocol::Message;
use crate::Result;
use tokio::io::AsyncWriteExt;

pub trait Node {
    /// Handle a message and optionally send a reply.
    /// Return `Ok(true)` if the message was handled, `Ok(false)` to fall back to default handling.
    fn handle_message(
        &mut self,
        msg: Message,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    fn run(&mut self) -> impl std::future::Future<Output = Result<()>>;
}

/// It is concrete struct that encapsulates shared
/// state and behavior common to all Maelstrom node implementations.
/// Other specific node reuse it via composition, delegate common feature to it.
#[derive(Debug)]
pub struct BaseNode {
    pub node_id: String,
    pub node_ids: Vec<String>,
    msg_counter: usize,
    pub output: tokio::io::Stdout,
}

impl BaseNode {
    pub fn new() -> Self {
        Self {
            node_id: String::new(),
            node_ids: Vec::new(),
            msg_counter: 1, // start at 1 for msg_id
            output: tokio::io::stdout(),
        }
    }

    pub fn next_msg_id(&mut self) -> usize {
        let id = self.msg_counter;
        self.msg_counter += 1;
        id
    }

    pub fn handle_init(&mut self, node_id: &str, node_ids: &Vec<String>) {
        self.node_id = node_id.to_string();
        self.node_ids = node_ids.clone();
    }

    pub async fn send_msg_to_output(&mut self, msg: Message) -> Result<()> {
        let json = serde_json::to_string(&msg)?;

        self.output
            .write_all(format!("{}\n", json).as_bytes())
            .await?;
        Ok(())
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
