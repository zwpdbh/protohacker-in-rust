use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
use std::io::StdoutLock;

/// Use composition over inheritance
pub struct UniqueIdsNode {
    base: BaseNode,
    id_gen: IdGenerator,
}

impl UniqueIdsNode {
    pub fn new() -> Self {
        // In real impl, you'd use logical clock or coordination
        // For now, just a simple counter
        Self {
            base: BaseNode::new(),
            id_gen: IdGenerator::new(),
        }
    }
}

impl Node for UniqueIdsNode {
    fn handle_message(&mut self, msg: Message, output: &mut StdoutLock) -> Result<()> {
        match msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        msg_id: Some(self.base.next_msg_id()),
                        payload: Payload::InitOk {
                            in_reply_to: msg.body.msg_id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
                Ok(())
            }
            Payload::Generate => {
                let unique_id = self.id_gen.next_id(&self.base.node_id);
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        msg_id: Some(self.base.next_msg_id()),
                        payload: Payload::GenerateOk {
                            id: unique_id,
                            in_reply_to: msg.body.msg_id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
                Ok(())
            }
            other => Err(Error::Other(format!("{:?} should not happend", other))),
        }
    }
}
