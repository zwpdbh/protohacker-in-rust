use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Message {
    pub src: String,
    #[serde(rename = "dest")]
    pub dst: String,
    pub body: MessageBody,
}

impl Message {
    pub fn into_reply(&self, msg_id: Option<usize>, payload: Payload) -> Message {
        Message {
            src: self.dst.clone(),
            dst: self.src.clone(),
            body: MessageBody {
                msg_id,
                payload,
                in_reply_to: self.body.msg_id,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct MessageBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<usize>,
    // It should also associate itself with the original message
    // by setting the "in_reply_to" field to the original message ID.
    pub in_reply_to: Option<usize>,
    // Use `#[serde(flatten)]` make its fields are merged into the body object instead of nested
    // And the `serde(tag = "type")` on `Payload` will embed flattened value into "type"
    #[serde(flatten)]
    pub payload: Payload,
}

/// Use a discriminant field named "type"
/// Flatten the variant’s fields into the same object.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Payload {
    Echo {
        echo: String,
    },
    EchoOk {
        echo: String,
    },
    /// At the start of a test, Maelstrom issues a init message to each node
    Init {
        /// It indicates the ID of the node which is receiving this message.
        /// The node should remember this ID and include as the `src` of any message it sends
        node_id: String,
        /// It lists all nodes in the cluster, including the recipient.
        node_ids: Vec<String>,
    },
    InitOk,
    Generate,
    GenerateOk {
        id: String,
    },
    Broadcast {
        message: usize,
    },
    BroadcastOk,
    Read,
    ReadOk {
        messages: Vec<usize>,
    },
    Topology {
        topology: HashMap<String, Vec<String>>,
    },
    TopologyOk,
    Gossip {
        messages: Vec<usize>,
    },
}

pub enum NodeEvent {
    External(Message),
    Internal(NodeMessage),
}

pub enum NodeMessage {
    Gossip,
}
