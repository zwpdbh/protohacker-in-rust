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
            body: MessageBody { msg_id, payload },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct MessageBody {
    pub msg_id: Option<usize>,

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
        in_reply_to: Option<usize>,
    },
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk {
        in_reply_to: Option<usize>,
    },
    Generate,
    GenerateOk {
        id: String,
        in_reply_to: Option<usize>,
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
