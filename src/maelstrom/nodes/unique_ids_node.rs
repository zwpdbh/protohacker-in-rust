use crate::maelstrom::*;
use crate::{Error, Result};
use std::io::{StdoutLock, Write};
pub struct UniqueIdsNode {
    pub id: String,
    pub msg_counter: usize,
    pub node_ids: Vec<String>,
}

impl UniqueIdsNode {
    pub fn new() -> Self {
        UniqueIdsNode {
            id: "".to_string(),
            msg_counter: 0,
            node_ids: vec![],
        }
    }
    pub fn handle(&mut self, input: Message, output: &mut StdoutLock) -> Result<()> {
        match input.body.payload {
            Payload::Init { node_id, node_ids } => {
                self.id = node_id;
                self.node_ids = node_ids;

                let reply = Message {
                    src: input.dst,
                    dst: input.src,
                    body: MessageBody {
                        id: Some(self.msg_counter),
                        payload: Payload::InitOk {
                            in_reply_to: input.body.id,
                        },
                    },
                };

                let _ = self.send_reply(&reply, output)?;
            }
            Payload::Echo { echo } => {
                let reply = Message {
                    src: input.dst,
                    dst: input.src,
                    body: MessageBody {
                        id: Some(self.msg_counter),
                        payload: Payload::EchoOk {
                            echo,
                            in_reply_to: input.body.id,
                        },
                    },
                };
                let _ = self.send_reply(&reply, output)?;
            }
            Payload::EchoOk { .. } => {}
            other => {
                return Err(Error::Other(format!("{:?} should not reach here", other)));
            }
        }

        Ok(())
    }

    fn send_reply(&mut self, msg: &Message, output: &mut StdoutLock) -> Result<()> {
        let _ = serde_json::to_writer(&mut *output, msg)
            .map_err(|e| Error::Other(format!("failed to serde reply: {}", e)))?;
        let _ = output.write_all(b"\n")?;

        self.msg_counter += 1;
        Ok(())
    }
}
