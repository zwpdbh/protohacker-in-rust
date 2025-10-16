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

#[derive(Debug, PartialEq)]
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
        roads: Vec<u16>,
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
                dst.put_u8(0x40);
                dst.put_u32(interval);
            }
            Message::Heartbeat => {
                dst.put_u8(0x41);
            }
            Message::IAmCamera { road, mile, limit } => {
                dst.put_u8(0x80);
                dst.put_u16(road);
                dst.put_u16(mile);
                dst.put_u16(limit);
            }
            Message::IAmDispatcher { numroads, roads } => {
                dst.put_u8(0x81);
                dst.put_u8(numroads);
                for road in roads {
                    dst.put_u16(road);
                }
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
mod encode_tests {
    use super::*;
    use bytes::BytesMut;

    fn msg_str(s: &str) -> MessageStr {
        s.into()
    }

    // === 0x10: Error ===
    #[test]
    fn encode_error_bad() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Error {
                    msg: msg_str("bad"),
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x10, 0x03, b'b', b'a', b'd']);
    }

    #[test]
    fn encode_error_illegal_msg() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Error {
                    msg: msg_str("illegal msg"),
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[
                0x10, 0x0b, b'i', b'l', b'l', b'e', b'g', b'a', b'l', b' ', b'm', b's', b'g'
            ]
        );
    }

    // === 0x20: Plate ===
    #[test]
    fn encode_plate_un1x_1000() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Plate {
                    plate: msg_str("UN1X"),
                    timestamp: 1000,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[0x20, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x00, 0x03, 0xe8]
        );
    }

    #[test]
    fn encode_plate_re05bkg_123456() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Plate {
                    plate: msg_str("RE05BKG"),
                    timestamp: 123456,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[
                0x20, 0x07, b'R', b'E', b'0', b'5', b'B', b'K', b'G', 0x00, 0x01, 0xe2, 0x40
            ]
        );
    }

    // === 0x21: Ticket ===
    #[test]
    fn encode_ticket_un1x() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Ticket {
                    plate: msg_str("UN1X"),
                    road: 66,
                    mile1: 100,
                    timestamp1: 123456,
                    mile2: 110,
                    timestamp2: 123816,
                    speed: 10000,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[
                0x21, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x42, 0x00, 0x64, 0x00, 0x01, 0xe2, 0x40,
                0x00, 0x6e, 0x00, 0x01, 0xe3, 0xa8, 0x27, 0x10
            ]
        );
    }

    #[test]
    fn encode_ticket_re05bkg() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::Ticket {
                    plate: msg_str("RE05BKG"),
                    road: 368,
                    mile1: 1234,
                    timestamp1: 1000000,
                    mile2: 1235,
                    timestamp2: 1000060,
                    speed: 6000,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[
                0x21, 0x07, b'R', b'E', b'0', b'5', b'B', b'K', b'G', 0x01, 0x70, 0x04, 0xd2, 0x00,
                0x0f, 0x42, 0x40, 0x04, 0xd3, 0x00, 0x0f, 0x42, 0x7c, 0x17, 0x70
            ]
        );
    }

    // === 0x40: WantHeartbeat ===
    #[test]
    fn encode_want_heartbeat_10() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(Message::WantHeartbeat { interval: 10 }, &mut buf)
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x40, 0x00, 0x00, 0x00, 0x0a]);
    }

    #[test]
    fn encode_want_heartbeat_1243() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(Message::WantHeartbeat { interval: 1243 }, &mut buf)
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x40, 0x00, 0x00, 0x04, 0xdb]);
    }

    // === 0x41: Heartbeat ===
    #[test]
    fn encode_heartbeat() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec.encode(Message::Heartbeat, &mut buf).unwrap();
        assert_eq!(buf.as_ref(), &[0x41]);
    }

    // === 0x80: IAmCamera ===
    #[test]
    fn encode_i_am_camera_66() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::IAmCamera {
                    road: 66,
                    mile: 100,
                    limit: 60,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x80, 0x00, 0x42, 0x00, 0x64, 0x00, 0x3c]);
    }

    #[test]
    fn encode_i_am_camera_368() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::IAmCamera {
                    road: 368,
                    mile: 1234,
                    limit: 40,
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x80, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x28]);
    }

    // === 0x81: IAmDispatcher ===
    #[test]
    fn encode_i_am_dispatcher_single() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::IAmDispatcher {
                    numroads: 1,
                    roads: vec![66],
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf.as_ref(), &[0x81, 0x01, 0x00, 0x42]);
    }

    #[test]
    fn encode_i_am_dispatcher_multi() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec
            .encode(
                Message::IAmDispatcher {
                    numroads: 3,
                    roads: vec![66, 368, 5000],
                },
                &mut buf,
            )
            .unwrap();
        assert_eq!(
            buf.as_ref(),
            &[0x81, 0x03, 0x00, 0x42, 0x01, 0x70, 0x13, 0x88]
        );
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;
    use bytes::BytesMut;

    // Helper to avoid .unwrap() noise in tests
    fn decode_single(mut codec: MessageCodec, data: &[u8]) -> Message {
        let mut buf = BytesMut::from(data);
        let result = codec.decode(&mut buf).unwrap();
        assert!(buf.is_empty(), "All bytes should be consumed");
        result.unwrap()
    }

    // === 0x10: Error ===
    #[test]
    fn decode_error_bad() {
        let msg = decode_single(MessageCodec::new(), &[0x10, 0x03, b'b', b'a', b'd']);
        assert_eq!(msg, Message::Error { msg: "bad".into() });
    }

    #[test]
    fn decode_error_illegal_msg() {
        let data = &[
            0x10, 0x0b, b'i', b'l', b'l', b'e', b'g', b'a', b'l', b' ', b'm', b's', b'g',
        ];
        let msg = decode_single(MessageCodec::new(), data);
        assert_eq!(
            msg,
            Message::Error {
                msg: "illegal msg".into()
            }
        );
    }

    // === 0x20: Plate ===
    #[test]
    fn decode_plate_un1x_1000() {
        let data = &[0x20, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x00, 0x03, 0xe8];
        let msg = decode_single(MessageCodec::new(), data);
        assert_eq!(
            msg,
            Message::Plate {
                plate: "UN1X".into(),
                timestamp: 1000
            }
        );
    }

    #[test]
    fn decode_plate_re05bkg_123456() {
        let data = &[
            0x20, 0x07, b'R', b'E', b'0', b'5', b'B', b'K', b'G', 0x00, 0x01, 0xe2, 0x40,
        ];
        let msg = decode_single(MessageCodec::new(), data);
        assert_eq!(
            msg,
            Message::Plate {
                plate: "RE05BKG".into(),
                timestamp: 123456
            }
        );
    }

    // === 0x21: Ticket ===
    #[test]
    fn decode_ticket_un1x() {
        let data = &[
            0x21, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x42, 0x00, 0x64, 0x00, 0x01, 0xe2, 0x40,
            0x00, 0x6e, 0x00, 0x01, 0xe3, 0xa8, 0x27, 0x10,
        ];
        let msg = decode_single(MessageCodec::new(), data);
        assert_eq!(
            msg,
            Message::Ticket {
                plate: "UN1X".into(),
                road: 66,
                mile1: 100,
                timestamp1: 123456,
                mile2: 110,
                timestamp2: 123816,
                speed: 10000,
            }
        );
    }

    #[test]
    fn decode_ticket_re05bkg() {
        let data = &[
            0x21, 0x07, b'R', b'E', b'0', b'5', b'B', b'K', b'G', 0x01, 0x70, 0x04, 0xd2, 0x00,
            0x0f, 0x42, 0x40, 0x04, 0xd3, 0x00, 0x0f, 0x42, 0x7c, 0x17, 0x70,
        ];
        let msg = decode_single(MessageCodec::new(), data);
        assert_eq!(
            msg,
            Message::Ticket {
                plate: "RE05BKG".into(),
                road: 368,
                mile1: 1234,
                timestamp1: 1000000,
                mile2: 1235,
                timestamp2: 1000060,
                speed: 6000,
            }
        );
    }

    // === 0x40: WantHeartbeat ===
    #[test]
    fn decode_want_heartbeat_10() {
        let msg = decode_single(MessageCodec::new(), &[0x40, 0x00, 0x00, 0x00, 0x0a]);
        assert_eq!(msg, Message::WantHeartbeat { interval: 10 });
    }

    #[test]
    fn decode_want_heartbeat_1243() {
        let msg = decode_single(MessageCodec::new(), &[0x40, 0x00, 0x00, 0x04, 0xdb]);
        assert_eq!(msg, Message::WantHeartbeat { interval: 1243 });
    }

    // === 0x41: Heartbeat ===
    #[test]
    fn decode_heartbeat() {
        let msg = decode_single(MessageCodec::new(), &[0x41]);
        assert_eq!(msg, Message::Heartbeat);
    }

    // === 0x80: IAmCamera ===
    #[test]
    fn decode_i_am_camera_66() {
        let msg = decode_single(
            MessageCodec::new(),
            &[0x80, 0x00, 0x42, 0x00, 0x64, 0x00, 0x3c],
        );
        assert_eq!(
            msg,
            Message::IAmCamera {
                road: 66,
                mile: 100,
                limit: 60,
            }
        );
    }

    #[test]
    fn decode_i_am_camera_368() {
        let msg = decode_single(
            MessageCodec::new(),
            &[0x80, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x28],
        );
        assert_eq!(
            msg,
            Message::IAmCamera {
                road: 368,
                mile: 1234,
                limit: 40,
            }
        );
    }

    // === 0x81: IAmDispatcher ===
    #[test]
    fn decode_i_am_dispatcher_single() {
        let msg = decode_single(MessageCodec::new(), &[0x81, 0x01, 0x00, 0x42]);
        assert_eq!(
            msg,
            Message::IAmDispatcher {
                numroads: 1,
                roads: vec![66],
            }
        );
    }

    #[test]
    fn decode_i_am_dispatcher_multi() {
        let msg = decode_single(
            MessageCodec::new(),
            &[0x81, 0x03, 0x00, 0x42, 0x01, 0x70, 0x13, 0x88],
        );
        assert_eq!(
            msg,
            Message::IAmDispatcher {
                numroads: 3,
                roads: vec![66, 368, 5000],
            }
        );
    }

    // === Partial / Streaming Decoding (Optional but Recommended) ===
    #[test]
    fn decode_partial_heartbeat() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from(&[0x41][..1]); // only 1 byte
        assert!(codec.decode(&mut buf).unwrap().is_some()); // should decode immediately
    }

    #[test]
    fn decode_partial_plate_needs_more() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from(&[0x20, 0x04][..]); // has tag + len, but no string yet
        assert!(codec.decode(&mut buf).unwrap().is_none()); // not enough for "UN1X"
    }
}
