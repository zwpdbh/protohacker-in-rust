use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};
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
    fn handle_message(&mut self, msg: &Message, output: &mut StdoutLock) -> Result<()> {
        match &msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);

                let reply = msg.into_reply(Some(self.base.next_msg_id()), Payload::InitOk);

                self.send_reply(reply, output)?;
                Ok(())
            }
            Payload::Echo { echo } => {
                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::EchoOk { echo: echo.into() },
                );

                self.send_reply(reply, output)?;
                Ok(())
            }
            Payload::EchoOk { .. } => Ok(()), // ignore
            other => Err(Error::Other(format!("{:?} should not happend", other))), // not handled
        }
    }
}
