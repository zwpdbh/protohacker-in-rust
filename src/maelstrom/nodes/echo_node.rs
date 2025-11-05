use crate::Result;
use crate::maelstrom::node::*;
use crate::maelstrom::*;
use std::io::StdoutLock;

pub struct EchoNode {
    // composition ver inheritance, has a BaseNode
    // Traits define behavior, not shared state.
    base: BaseNode,
}

impl EchoNode {
    pub fn new() -> Self {
        Self {
            base: BaseNode::new(),
        }
    }
}

impl Node for EchoNode {
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
            Payload::Echo { echo } => {
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        id: Some(self.base.next_msg_id()),
                        payload: Payload::EchoOk {
                            echo,
                            in_reply_to: msg.body.id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
                Ok(true)
            }
            Payload::EchoOk { .. } => Ok(true), // ignore
            _ => Ok(false),                     // not handled
        }
    }
}
