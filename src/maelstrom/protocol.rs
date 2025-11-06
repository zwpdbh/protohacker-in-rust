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
    Init {
        node_id: String,
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
}
