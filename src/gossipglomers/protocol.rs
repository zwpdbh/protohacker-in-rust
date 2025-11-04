use serde::{Deserialize, Serialize};

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub src: String,
    #[serde(rename = "dest")]
    pub dst: String,
    pub body: MessageBody,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MessageBody {
    #[serde(rename = "msg_id")]
    pub id: Option<usize>,

    // Use `#[serde(flatten)]` make its fields are merged into the body object instead of nested
    #[serde(flatten)]
    pub message_type: MessageType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageType {
    Read {
        key: usize,
    },
    ReadOk {
        value: usize,
        in_reply_to: usize,
    },
    Init {
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk {
        in_reply_to: usize,
    },
}
