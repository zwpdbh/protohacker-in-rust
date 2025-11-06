use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
use std::collections::HashMap;
use std::io::StdoutLock;
use tracing::error;

pub struct BroadcastNode {
    base: BaseNode,
    id_gen: IdGenerator,
    topology: HashMap<String, Vec<String>>,
}

impl BroadcastNode {
    pub fn new() -> Self {
        Self {
            base: BaseNode::new(),
            id_gen: IdGenerator::new(),
            topology: HashMap::new(),
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
                        msg_id: Some(self.base.next_msg_id()),
                        payload: Payload::InitOk {
                            in_reply_to: msg.body.msg_id,
                        },
                    },
                };
                self.send_reply(reply, output)?;
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
            }
            Payload::Topology { topology } => {
                self.topology = topology;
                let reply = Message {
                    src: msg.dst,
                    dst: msg.src,
                    body: MessageBody {
                        msg_id: Some(self.base.next_msg_id()),
                        payload: Payload::TopologyOk,
                    },
                };

                self.send_reply(reply, output)?;
            }
            other => {
                let error_msg = format!("{:?} should not happend", other);
                error!(error_msg);
                return Err(Error::Other(error_msg));
            }
        }
        Ok(())
    }
}
