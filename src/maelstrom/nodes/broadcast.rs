use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
use std::io::StdoutLock;

pub struct BroadcastNode {
    base: BaseNode,
    id_gen: IdGenerator,
}

impl BroadcastNode {
    pub fn new() -> Self {
        Self {
            base: BaseNode::new(),
            id_gen: IdGenerator::new(),
        }
    }
}

impl Node for BroadcastNode {
    fn handle_message(&mut self, msg: Message, output: &mut StdoutLock) -> Result<()> {
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
            }
            Payload::Generate => {
                let id = self.id_gen.next_id(&self.base.id);
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
            }
            other => return Err(Error::Other(format!("{:?} should not happend", other))),
        }
        Ok(())
    }
}
