use crate::maelstrom::node::*;
use crate::maelstrom::*;
use crate::{Error, Result};

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
    async fn handle_message(&mut self, msg: Message) -> Result<()> {
        match &msg.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.base.handle_init(node_id, node_ids);

                let reply = msg.into_reply(Some(self.base.next_msg_id()), Payload::InitOk);

                self.base.send_msg_to_output(reply).await?;
                Ok(())
            }
            Payload::Echo { echo } => {
                let reply = msg.into_reply(
                    Some(self.base.next_msg_id()),
                    Payload::EchoOk { echo: echo.into() },
                );

                self.base.send_msg_to_output(reply).await?;
                Ok(())
            }
            Payload::EchoOk { .. } => Ok(()), // ignore
            other => Err(Error::Other(format!("{:?} should not happend", other))), // not handled
        }
    }

    async fn run(&mut self) -> Result<()> {
        let stdin = std::io::stdin();

        let deserializer = serde_json::Deserializer::from_reader(stdin.lock());
        let mut stream = deserializer.into_iter::<Message>();

        while let Some(result) = stream.next() {
            let msg = result?;
            let _ = self.handle_message(msg).await?;
        }

        Ok(())
    }
}
