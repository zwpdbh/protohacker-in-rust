#![allow(unused)]
use crate::{Error, Result};
use bincode::Decode;
use bincode::Encode;
use bytes::BufMut;
use bytes::{Bytes, BytesMut};

use bytes::Buf;
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

// At the top of your file (or in a `const` block inside impl if preferred)
const U8_SIZE: usize = 1;
const U16_SIZE: usize = 2;
const U32_SIZE: usize = 4;

// Message tag constants (optional but improves clarity)
const TAG_ERROR: u8 = 0x10;
const TAG_PLATE: u8 = 0x20;
const TAG_TICKET: u8 = 0x21;
const TAG_WANT_HEARTBEAT: u8 = 0x40;
const TAG_HEARTBEAT: u8 = 0x41;
const TAG_I_AM_CAMERA: u8 = 0x80;
const TAG_I_AM_DISPATCHER: u8 = 0x81;

// Fixed sizes for compound messages
const PLATE_FIXED_SIZE: usize = U32_SIZE; // timestamp
const TICKET_FIXED_SIZE: usize = U16_SIZE + // road
    U16_SIZE + // mile1
    U32_SIZE + // timestamp1
    U16_SIZE + // mile2
    U32_SIZE + // timestamp2
    U16_SIZE; // speed

const I_AM_CAMERA_SIZE: usize = U16_SIZE + // road
    U16_SIZE + // mile
    U16_SIZE; // limit

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
                dst.put_u8(TAG_ERROR);
                str_codec.encode(msg, dst)?;
            }
            Message::Plate { plate, timestamp } => {
                dst.put_u8(TAG_PLATE);
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
                dst.put_u8(TAG_TICKET);
                str_codec.encode(plate, dst)?;
                dst.put_u16(road);
                dst.put_u16(mile1);
                dst.put_u32(timestamp1);
                dst.put_u16(mile2);
                dst.put_u32(timestamp2);
                dst.put_u16(speed);
            }
            Message::WantHeartbeat { interval } => {
                dst.put_u8(TAG_WANT_HEARTBEAT);
                dst.put_u32(interval);
            }
            Message::Heartbeat => {
                dst.put_u8(TAG_HEARTBEAT);
            }
            Message::IAmCamera { road, mile, limit } => {
                dst.put_u8(TAG_I_AM_CAMERA);
                dst.put_u16(road);
                dst.put_u16(mile);
                dst.put_u16(limit);
            }
            Message::IAmDispatcher { numroads, roads } => {
                dst.put_u8(TAG_I_AM_DISPATCHER);
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

    // Private helper function to check if enough bytes are available
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < U8_SIZE {
            return Ok(None);
        }

        let tag = src[0];
        let mut offset = U8_SIZE; // consumed 1 byte for tag

        // Helper to decode MessageStr
        fn decode_message_str(
            src: &BytesMut,
            offset: usize,
        ) -> Result<(Option<MessageStr>, usize)> {
            if src.len() < offset + U8_SIZE {
                return Ok((None, offset));
            }
            let len = src[offset] as usize;
            if src.len() < offset + U8_SIZE + len {
                return Ok((None, offset));
            }

            let slice_start = offset;
            let slice_end = offset + U8_SIZE + len;
            let mut temp_buf = BytesMut::from(&src[slice_start..slice_end]);

            let mut str_codec = MessageStrCodec::new();
            match str_codec.decode(&mut temp_buf)? {
                Some(msg) => {
                    if !temp_buf.is_empty() {
                        return Err(crate::Error::General(
                            "MessageStrCodec left unconsumed bytes".into(),
                        ));
                    }
                    Ok((Some(msg), slice_end))
                }
                None => Ok((None, offset)),
            }
        }

        let message = match tag {
            TAG_ERROR => {
                let (msg_opt, new_offset) = decode_message_str(src, offset)?;
                if msg_opt.is_none() {
                    return Ok(None);
                }
                offset = new_offset;
                Message::Error {
                    msg: msg_opt.unwrap(),
                }
            }
            TAG_PLATE => {
                let (plate_opt, new_offset) = decode_message_str(src, offset)?;
                if plate_opt.is_none() {
                    return Ok(None);
                }
                offset = new_offset;
                if src.len() < offset + PLATE_FIXED_SIZE {
                    return Ok(None);
                }
                let timestamp =
                    u32::from_be_bytes(src[offset..offset + U32_SIZE].try_into().unwrap());
                offset += U32_SIZE;
                Message::Plate {
                    plate: plate_opt.unwrap(),
                    timestamp,
                }
            }
            TAG_TICKET => {
                let (plate_opt, new_offset) = decode_message_str(src, offset)?;
                if plate_opt.is_none() {
                    return Ok(None);
                }
                offset = new_offset;
                if src.len() < offset + TICKET_FIXED_SIZE {
                    return Ok(None);
                }

                let road = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                let mile1 = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                let timestamp1 =
                    u32::from_be_bytes(src[offset..offset + U32_SIZE].try_into().unwrap());
                offset += U32_SIZE;
                let mile2 = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                let timestamp2 =
                    u32::from_be_bytes(src[offset..offset + U32_SIZE].try_into().unwrap());
                offset += U32_SIZE;
                let speed = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;

                Message::Ticket {
                    plate: plate_opt.unwrap(),
                    road,
                    mile1,
                    timestamp1,
                    mile2,
                    timestamp2,
                    speed,
                }
            }
            TAG_WANT_HEARTBEAT => {
                if src.len() < offset + U32_SIZE {
                    return Ok(None);
                }
                let interval =
                    u32::from_be_bytes(src[offset..offset + U32_SIZE].try_into().unwrap());
                offset += U32_SIZE;
                Message::WantHeartbeat { interval }
            }
            TAG_HEARTBEAT => Message::Heartbeat,
            TAG_I_AM_CAMERA => {
                if src.len() < offset + I_AM_CAMERA_SIZE {
                    return Ok(None);
                }
                let road = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                let mile = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                let limit = u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                offset += U16_SIZE;
                Message::IAmCamera { road, mile, limit }
            }
            TAG_I_AM_DISPATCHER => {
                if src.len() < offset + U8_SIZE {
                    return Ok(None);
                }
                let numroads = src[offset];
                offset += U8_SIZE;
                let roads_len = numroads as usize * U16_SIZE;
                if src.len() < offset + roads_len {
                    return Ok(None);
                }

                let mut roads = Vec::with_capacity(numroads as usize);
                for _ in 0..numroads {
                    let road =
                        u16::from_be_bytes(src[offset..offset + U16_SIZE].try_into().unwrap());
                    offset += U16_SIZE;
                    roads.push(road);
                }
                Message::IAmDispatcher { numroads, roads }
            }
            _ => {
                return Err(crate::Error::General(format!(
                    "Unknown message tag: 0x{:02x}",
                    tag
                )));
            }
        };

        src.advance(offset);
        Ok(Some(message))
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
