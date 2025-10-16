#![allow(unused)]
use crate::{Error, Result};
use bincode::Decode;
use bincode::Encode;
use bytes::BufMut;
use bytes::{Bytes, BytesMut};

use std::str::FromStr;
use tokio_util::codec::LengthDelimitedCodec;
use tokio_util::codec::{Decoder, Encoder};
// A string of characters in a length-prefixed format.
// A str is transmitted as a single u8 containing the string's length (0 to 255), followed by that many bytes of u8, in order, containing ASCII character codes.
#[derive(Debug, Encode, Decode, PartialEq, Clone)]
pub struct MessageStr {
    inner: String,
}

pub struct MessageStrCodec {
    inner: LengthDelimitedCodec,
}

impl MessageStrCodec {
    pub fn new() -> Self {
        Self {
            inner: LengthDelimitedCodec::builder()
                .length_field_length(1) // u64
                .length_field_type::<u8>()
                .big_endian()
                .max_frame_length(255) // 1 MB max
                .new_codec(),
        }
    }
}

// You must implement Encoder<MessageStr> and Decoder because:
// tokio_util::codec::LengthDelimitedCodec only works with raw Bytes — not your custom types like MessageStr.
// So you need a bridge between:
// Your high-level type (MessageStr)
// The low-level framing (LengthDelimitedCodec that handles [len][data...])
// That bridge is your manual Encoder/Decoder impl.
impl Encoder<MessageStr> for MessageStrCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: MessageStr, dst: &mut BytesMut) -> Result<()> {
        let bytes = item.inner.as_bytes();
        if bytes.len() > 255 {
            return Err(crate::Error::General("String too long".into()));
        }
        // Encode RAW bytes — no bincode, no JSON
        self.inner
            .encode(Bytes::copy_from_slice(bytes), dst)
            .map_err(|e| crate::Error::General(e.to_string()))
    }
}

impl Decoder for MessageStrCodec {
    type Error = crate::Error;
    type Item = MessageStr;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        match self.inner.decode(src)? {
            Some(bytes) => {
                if !bytes.is_ascii() {
                    return Err(crate::Error::General("Non-ASCII string".into()));
                }
                let s = String::from_utf8(bytes.to_vec())
                    .map_err(|e| crate::Error::General(e.to_string()))?;
                Ok(Some(MessageStr { inner: s }))
            }
            None => Ok(None),
        }
    }
}

impl From<&str> for MessageStr {
    fn from(s: &str) -> Self {
        MessageStr {
            inner: s.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Error {
        msg: MessageStr,
    },
    Plate {
        plate: MessageStr,
        timestamp: u32,
    },
    Ticket {
        plate: MessageStr,
        road: u16,
        mile1: u16,
        timestamp1: u32,
        mile2: u16,
        timestamp2: u32,
        speed: u16,
    },
    WantHeartbeat {
        interval: u32,
    },
    Heartbeat,
    IAmCamera {
        road: u16,
        mile: u16,
        limit: u16,
    },
    IAmDispatcher {
        numroads: u8,
        road: Vec<u16>,
    },
}

#[derive(Debug)]
pub struct MessageCodec;

impl MessageCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<()> {
        let mut str_codec = MessageStrCodec::new();

        match item {
            Message::Error { msg } => {
                dst.put_u8(0x10);
                str_codec.encode(msg, dst)?;
            }
            Message::Plate { plate, timestamp } => {
                dst.put_u8(0x20);
                str_codec.encode(plate, dst)?;
                dst.put_u32(timestamp);
            }
            Message::Ticket {
                plate,
                road,
                mile1,
                timestamp1,
                mile2,
                timestamp2,
                speed,
            } => {
                dst.put_u8(0x21);
                str_codec.encode(plate, dst)?;
                dst.put_u16(road);
                dst.put_u16(mile1);
                dst.put_u32(timestamp1);
                dst.put_u16(mile2);
                dst.put_u32(timestamp2);
                dst.put_u16(speed);
            }
            Message::WantHeartbeat { interval } => {
                todo!()
            }
            Message::Heartbeat => {
                todo!()
            }
            Message::IAmCamera { road, mile, limit } => {
                todo!()
            }
            Message::IAmDispatcher { numroads, road } => {
                todo!()
            }
        }
        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Error = crate::Error;
    type Item = Message;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        return Ok(None);
    }
}

#[cfg(test)]
mod message_str_tests {
    #![allow(unused)]
    use super::*;

    #[test]
    fn case01() -> Result<()> {
        let mut codec = MessageStrCodec::new();
        let mut buffer = BytesMut::new();

        codec.encode("foo".into(), &mut buffer)?;
        // Expected: [03][66 6f 6f] → hex: 03 66 6f 6f
        // or [3, 102, 111, 111]
        let expected = vec![0x03, b'f', b'o', b'o'];
        assert_eq!(buffer.as_ref(), expected.as_slice());

        let decoded_msg = codec.decode(&mut buffer)?.unwrap();
        assert_eq!(decoded_msg, "foo".into());

        Ok(())
    }

    #[test]
    fn case02() -> Result<()> {
        let mut codec = MessageStrCodec::new();
        let mut buffer = BytesMut::new();
        codec.encode("".into(), &mut buffer)?;

        let expected = vec![0x03];
        assert_eq!(buffer.as_ref(), expected.as_slice());

        let decoded_msg = codec.decode(&mut buffer)?.unwrap();
        assert_eq!(decoded_msg, "".into());

        Ok(())
    }
}

#[cfg(test)]
mod message_tests {
    #![allow(unused)]
    use super::*;
    use anyhow::{Ok, Result};

    #[test]
    fn case_error() -> Result<()> {
        // Test case 1
        let mut codec = MessageCodec::new();
        let mut buffer = BytesMut::new();

        let msg = Message::Error { msg: "bad".into() };

        codec.encode(msg, &mut buffer)?;
        let expected = vec![0x10, 0x03, b'b', b'a', b'd'];
        assert_eq!(buffer.as_ref(), expected.as_slice());

        Ok(())
    }

    #[test]
    fn case_plate() -> Result<()> {
        // Test case 1
        let mut codec = MessageCodec::new();
        let mut buffer = BytesMut::new();

        let msg = Message::Plate {
            plate: "UN1X".into(),
            timestamp: 1000,
        };

        codec.encode(msg, &mut buffer)?;

        let expected = vec![0x20, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x00, 0x03, 0xe8];
        assert_eq!(buffer.as_ref(), expected.as_slice());

        Ok(())
    }
}
