use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};

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
    async fn handle_message(&mut self, msg: Message) -> Result<()> {
        match &msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(&node_id, &node_ids);

                let reply = msg.into_reply(Some(self.base.next_msg_id()), Payload::InitOk);

                self.base.send_msg_to_output(reply).await?;
                Ok(())
            }
            Payload::Generate => {
                let unique_id = self.id_gen.next_id(&self.base.node_id);

                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::GenerateOk { id: unique_id },
                );

                self.base.send_msg_to_output(reply).await?;
                Ok(())
            }
            other => Err(Error::Other(format!("{:?} should not happend", other))),
        }
    }
}
