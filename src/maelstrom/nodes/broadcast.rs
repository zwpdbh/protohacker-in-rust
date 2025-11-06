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
    fn handle_message(&mut self, msg: &Message, output: &mut StdoutLock) -> Result<()> {
        match &msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);

                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::InitOk {
                        in_reply_to: msg.body.msg_id,
                    },
                );

                self.send_reply(reply, output)?;
            }
            Payload::Generate => {
                let unique_id = self.id_gen.next_id(&self.base.node_id);
                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::GenerateOk {
                        id: unique_id,
                        in_reply_to: msg.body.msg_id,
                    },
                );

                self.send_reply(reply, output)?;
            }
            Payload::Topology { topology } => {
                self.topology = topology.clone();
                let reply = msg.into_reply(None, Payload::TopologyOk);

                self.send_reply(reply, output)?;
            }
            Payload::Broadcast { message } => {
                self.messages.push(*message);
                let reply = msg.into_reply(None, Payload::BroadcastOk);

                self.send_reply(reply, output)?;
            }
            Payload::Read => {
                let reply = msg.into_reply(
                    None,
                    Payload::ReadOk {
                        messages: self.messages.clone(),
                    },
                );
                self.send_reply(reply, output)?;
            }
            Payload::TopologyOk | Payload::BroadcastOk => {}
            other => {
                let error_msg = format!("{:?} should not happend", other);
                error!(error_msg);
                return Err(Error::Other(error_msg));
            }
        }
        Ok(())
    }
}
