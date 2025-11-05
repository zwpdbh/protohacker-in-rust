use crate::Result;
use crate::maelstrom::node::*;
use crate::maelstrom::*;
use std::io::StdoutLock;

/// Use composition over inheritance
pub struct UniqueIdsNode {
    base: BaseNode,
    /// extend the BaseNode feature by composite an additional counter
    counter: u64,
}

impl UniqueIdsNode {
    pub fn new() -> Self {
        // In real impl, you'd use logical clock or coordination
        // For now, just a simple counter
        Self {
            base: BaseNode::new(),
            counter: 1,
        }
    }

    fn next_unique_id(&mut self) -> String {
        self.counter += 1;
        format!("{}-{}", self.base.id, self.counter)
    }
}

impl Node for UniqueIdsNode {
    fn handle_message(&mut self, msg: Message, output: &mut StdoutLock) -> Result<bool> {
        match msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        id: Some(self.base.next_msg_id()),
                        payload: Payload::InitOk {
                            in_reply_to: msg.body.id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
                Ok(true)
            }
            Payload::Generate => {
                let id = self.next_unique_id();
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        id: Some(self.base.next_msg_id()),
                        payload: Payload::GenerateOk {
                            id,
                            in_reply_to: msg.body.id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
