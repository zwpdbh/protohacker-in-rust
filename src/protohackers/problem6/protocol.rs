use crate::protohackers::problem6::client::ClientId;
use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use super::client::*;

// =============================================================================
// PROTOCOL CONSTANTS
// =============================================================================

// Size constants
const U8_SIZE: usize = 1;
const U16_SIZE: usize = 2;
const U32_SIZE: usize = 4;

// Message type tags (first byte of each message)
const TAG_ERROR: u8 = 0x10;
const TAG_PLATE: u8 = 0x20;
const TAG_TICKET: u8 = 0x21;
const TAG_WANT_HEARTBEAT: u8 = 0x40;
const TAG_HEARTBEAT: u8 = 0x41;
const TAG_I_AM_CAMERA: u8 = 0x80;
const TAG_I_AM_DISPATCHER: u8 = 0x81;

// Fixed payload sizes (after variable-length fields)
const PLATE_FIXED_SIZE: usize = U32_SIZE; // timestamp only

const TICKET_FIXED_SIZE: usize = U16_SIZE   // road
    + U16_SIZE                              // mile1
    + U32_SIZE                              // timestamp1
    + U16_SIZE                              // mile2
    + U32_SIZE                              // timestamp2
    + U16_SIZE; // speed

const I_AM_CAMERA_SIZE: usize = U16_SIZE    // road
    + U16_SIZE                              // mile
    + U16_SIZE; // limit

// =============================================================================
// MESSAGE STR TYPE (Length-Prefixed String)
// =============================================================================

/// A protocol string: `[length: u8][ASCII bytes...]` (max 255 bytes).
///
/// This is a domain type that distinguishes protocol strings from regular
/// strings, ensuring they are always length-prefixed when encoded.
#[derive(Debug, PartialEq, Clone)]
pub struct MessageStr {
    inner: String,
}

impl MessageStr {
    /// Creates a new MessageStr after validating the string fits in 255 bytes.
    pub fn new(s: impl Into<String>) -> Result<Self> {
        let inner = s.into();
        if inner.len() > 255 {
            return Err(Error::Other(format!(
                "String too long: {} bytes (max 255)",
                inner.len()
            )));
        }
        Ok(Self { inner })
    }

    /// Returns the inner String.
    pub fn into_string(self) -> String {
        self.inner
    }

    /// Returns a string slice.
    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl From<&str> for MessageStr {
    fn from(s: &str) -> Self {
        // Note: This panics if s > 255 bytes. Use MessageStr::new() for validation.
        Self {
            inner: s.to_string(),
        }
    }
}

impl From<String> for MessageStr {
    fn from(inner: String) -> Self {
        Self { inner }
    }
}

impl From<MessageStr> for String {
    fn from(value: MessageStr) -> Self {
        value.inner
    }
}

// =============================================================================
// MESSAGE STR CODEC (Low-level framing for length-prefixed strings)
// =============================================================================

/// Codec for encoding/decoding length-prefixed strings.
///
/// Wire format: `[length: u8][content bytes...]` where length is 0-255.
/// Uses `LengthDelimitedCodec` internally to handle partial data buffering.
#[derive(Debug)]
pub struct MessageStrCodec {
    inner: LengthDelimitedCodec,
}

impl MessageStrCodec {
    pub fn new() -> Self {
        Self {
            inner: LengthDelimitedCodec::builder()
                .length_field_length(1) // 1 byte for length
                .length_field_type::<u8>() // u8 type
                .big_endian()
                .max_frame_length(255) // Max string length
                .new_codec(),
        }
    }
}

impl Default for MessageStrCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder<MessageStr> for MessageStrCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: MessageStr, dst: &mut BytesMut) -> Result<()> {
        let bytes = item.inner.into_bytes();
        // Note: LengthDelimitedCodec handles writing the length byte
        self.inner
            .encode(Bytes::from(bytes), dst)
            .map_err(|e| Error::Other(e.to_string()))
    }
}

impl Decoder for MessageStrCodec {
    type Error = crate::Error;
    type Item = MessageStr;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        match self.inner.decode(src)? {
            Some(bytes) => {
                if !bytes.is_ascii() {
                    return Err(Error::Other("Non-ASCII string in protocol message".into()));
                }
                let s =
                    String::from_utf8(bytes.to_vec()).map_err(|e| Error::Other(e.to_string()))?;
                Ok(Some(MessageStr::from(s)))
            }
            None => Ok(None), // Need more data
        }
    }
}

// =============================================================================
// MESSAGE TYPE (Protocol message enum)
// =============================================================================

/// All possible messages in the Speed Daemon protocol.
///
/// Messages are divided into:
/// - **On-wire messages**: Sent over TCP (Error, Plate, Ticket, etc.)
/// - **Internal messages**: Used for state management (Join, Leave, etc.)
#[derive(Debug, PartialEq, Clone)]
pub enum Message {
    // --- On-wire messages (encode/decode supported) ---
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

    // --- Internal messages (state channel only, no encode/decode) ---
    Join {
        client: Client,
    },
    Leave {
        client_id: ClientId,
    },
    DispatcherObservation {
        client_id: ClientId,
        roads: Vec<u16>,
    },
    PlateObservation {
        client_id: ClientId,
        road: u16,
        mile: u16,
        limit: u16,
        plate: String,
        timestamp: u32,
    },
}

// =============================================================================
// MESSAGE CODEC (High-level message framing)
// =============================================================================

/// Codec for encoding/decoding complete protocol messages.
///
/// This codec handles:
/// 1. Message type tag (1 byte)
/// 2. Variable-length string fields (via `MessageStrCodec`)
/// 3. Fixed-size integer fields (direct byte manipulation)
///
/// For decoding, it properly handles partial messages by returning `Ok(None)`
/// when more data is needed.
#[derive(Debug)]
pub struct MessageCodec {
    str_codec: MessageStrCodec,
}

impl MessageCodec {
    pub fn new() -> Self {
        Self {
            str_codec: MessageStrCodec::new(),
        }
    }

    /// Helper: Decodes a length-prefixed string from src starting at offset.
    /// Returns `Ok(None)` if more data is needed.
    fn decode_string_at(
        &mut self,
        src: &BytesMut,
        offset: usize,
    ) -> Result<Option<(MessageStr, usize)>> {
        // Create a temporary buffer containing just the length-prefixed string
        // This is needed because LengthDelimitedCodec expects the length at the start
        if src.len() < offset + U8_SIZE {
            return Ok(None); // Need at least the length byte
        }

        let len = src[offset] as usize;
        let total_str_bytes = U8_SIZE + len;

        if src.len() < offset + total_str_bytes {
            return Ok(None); // Need more string content
        }

        // Extract just the string frame and decode it
        let str_frame = &src[offset..offset + total_str_bytes];
        let mut temp_buf = BytesMut::from(str_frame);

        match self.str_codec.decode(&mut temp_buf)? {
            Some(msg_str) => Ok(Some((msg_str, offset + total_str_bytes))),
            None => {
                // This shouldn't happen if we calculated lengths correctly
                Err(Error::Other(
                    "String codec returned None despite sufficient data".into(),
                ))
            }
        }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = crate::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<()> {
        match item {
            Message::Error { msg } => {
                dst.put_u8(TAG_ERROR);
                self.str_codec.encode(msg, dst)?;
            }
            Message::Plate { plate, timestamp } => {
                dst.put_u8(TAG_PLATE);
                self.str_codec.encode(plate, dst)?;
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
                self.str_codec.encode(plate, dst)?;
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
            other => {
                return Err(Error::Other(format!(
                    "Cannot encode internal message: {:?}",
                    other
                )));
            }
        }
        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Error = crate::Error;
    type Item = Message;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        // Need at least 1 byte for the tag
        if src.len() < U8_SIZE {
            return Ok(None);
        }

        let tag = src[0];
        let mut offset = U8_SIZE; // Start after tag byte

        // Decode based on message type
        let message = match tag {
            TAG_ERROR => {
                let (msg, new_offset) = match self.decode_string_at(src, offset)? {
                    Some(result) => result,
                    None => return Ok(None), // Need more data
                };
                offset = new_offset;
                Message::Error { msg }
            }

            TAG_PLATE => {
                let (plate, new_offset) = match self.decode_string_at(src, offset)? {
                    Some(result) => result,
                    None => return Ok(None), // Need more data
                };
                offset = new_offset;

                // Check for timestamp
                if src.len() < offset + PLATE_FIXED_SIZE {
                    return Ok(None);
                }
                let timestamp = read_u32(src, &mut offset);
                Message::Plate { plate, timestamp }
            }

            TAG_TICKET => {
                let (plate, new_offset) = match self.decode_string_at(src, offset)? {
                    Some(result) => result,
                    None => return Ok(None), // Need more data
                };
                offset = new_offset;

                // Check for fixed fields
                if src.len() < offset + TICKET_FIXED_SIZE {
                    return Ok(None);
                }

                Message::Ticket {
                    plate,
                    road: read_u16(src, &mut offset),
                    mile1: read_u16(src, &mut offset),
                    timestamp1: read_u32(src, &mut offset),
                    mile2: read_u16(src, &mut offset),
                    timestamp2: read_u32(src, &mut offset),
                    speed: read_u16(src, &mut offset),
                }
            }

            TAG_WANT_HEARTBEAT => {
                if src.len() < offset + U32_SIZE {
                    return Ok(None);
                }
                Message::WantHeartbeat {
                    interval: read_u32(src, &mut offset),
                }
            }

            TAG_HEARTBEAT => Message::Heartbeat,

            TAG_I_AM_CAMERA => {
                if src.len() < offset + I_AM_CAMERA_SIZE {
                    return Ok(None);
                }
                Message::IAmCamera {
                    road: read_u16(src, &mut offset),
                    mile: read_u16(src, &mut offset),
                    limit: read_u16(src, &mut offset),
                }
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
                    roads.push(read_u16(src, &mut offset));
                }

                Message::IAmDispatcher { numroads, roads }
            }

            unknown => {
                return Err(Error::Other(format!(
                    "Unknown message tag: 0x{:02x}",
                    unknown
                )));
            }
        };

        // Advance the buffer past the consumed bytes
        src.advance(offset);
        Ok(Some(message))
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Reads a big-endian u16 from src at offset, then advances offset.
#[inline]
fn read_u16(src: &BytesMut, offset: &mut usize) -> u16 {
    let value = u16::from_be_bytes(src[*offset..*offset + U16_SIZE].try_into().unwrap());
    *offset += U16_SIZE;
    value
}

/// Reads a big-endian u32 from src at offset, then advances offset.
#[inline]
fn read_u32(src: &BytesMut, offset: &mut usize) -> u32 {
    let value = u32::from_be_bytes(src[*offset..*offset + U32_SIZE].try_into().unwrap());
    *offset += U32_SIZE;
    value
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod message_str_tests {
    use super::*;

    #[test]
    fn test_roundtrip_basic_string() -> Result<()> {
        let mut codec = MessageStrCodec::new();
        let mut buffer = BytesMut::new();

        let original: MessageStr = "foo".into();
        codec.encode(original.clone(), &mut buffer)?;

        // Check wire format: [length][content]
        assert_eq!(buffer.as_ref(), &[0x03, b'f', b'o', b'o']);

        let decoded = codec.decode(&mut buffer)?.unwrap();
        assert_eq!(decoded, original);
        Ok(())
    }

    #[test]
    fn test_empty_string() -> Result<()> {
        let mut codec = MessageStrCodec::new();
        let mut buffer = BytesMut::new();

        codec.encode("".into(), &mut buffer)?;
        assert_eq!(buffer.as_ref(), &[0x00]);

        let decoded = codec.decode(&mut buffer)?.unwrap();
        assert_eq!(decoded.as_str(), "");
        Ok(())
    }

    #[test]
    fn test_long_string_validation() {
        let long = "a".repeat(256);
        assert!(MessageStr::new(long).is_err());
    }

    #[test]
    fn test_non_ascii_rejected() {
        let mut codec = MessageStrCodec::new();
        // Manually craft a length-prefixed non-ASCII string
        let mut buf = BytesMut::from(&[0x03, 0xc0, 0xc1, 0xc2][..]);
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod message_codec_tests {
    use super::*;

    // Test helper
    fn msg_str(s: &str) -> MessageStr {
        s.into()
    }

    fn encode_message(msg: Message) -> BytesMut {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();
        buf
    }

    fn decode_message(data: &[u8]) -> Message {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from(data);
        let result = codec.decode(&mut buf).unwrap();
        assert!(buf.is_empty(), "Not all bytes were consumed");
        result.expect("Failed to decode message")
    }

    // === Error Message Tests ===
    #[test]
    fn test_error_roundtrip() {
        let original = Message::Error {
            msg: msg_str("bad"),
        };
        let encoded = encode_message(original.clone());
        assert_eq!(encoded.as_ref(), &[0x10, 0x03, b'b', b'a', b'd']);

        let decoded = decode_message(encoded.as_ref());
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_error_with_spaces() {
        let original = Message::Error {
            msg: msg_str("illegal msg"),
        };
        let encoded = encode_message(original);
        let decoded = decode_message(encoded.as_ref());
        assert_eq!(
            decoded,
            Message::Error {
                msg: msg_str("illegal msg")
            }
        );
    }

    // === Plate Message Tests ===
    #[test]
    fn test_plate_un1x() {
        let original = Message::Plate {
            plate: msg_str("UN1X"),
            timestamp: 1000,
        };
        let encoded = encode_message(original.clone());
        assert_eq!(
            encoded.as_ref(),
            &[0x20, 0x04, b'U', b'N', b'1', b'X', 0x00, 0x00, 0x03, 0xe8]
        );

        let decoded = decode_message(encoded.as_ref());
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_plate_re05bkg() {
        let original = Message::Plate {
            plate: msg_str("RE05BKG"),
            timestamp: 123456,
        };
        let encoded = encode_message(original);
        let decoded = decode_message(encoded.as_ref());
        assert_eq!(
            decoded,
            Message::Plate {
                plate: msg_str("RE05BKG"),
                timestamp: 123456,
            }
        );
    }

    // === Ticket Message Tests ===
    #[test]
    fn test_ticket_un1x() {
        let original = Message::Ticket {
            plate: msg_str("UN1X"),
            road: 66,
            mile1: 100,
            timestamp1: 123456,
            mile2: 110,
            timestamp2: 123816,
            speed: 10000,
        };
        let encoded = encode_message(original.clone());
        let decoded = decode_message(encoded.as_ref());
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_ticket_re05bkg() {
        let original = Message::Ticket {
            plate: msg_str("RE05BKG"),
            road: 368,
            mile1: 1234,
            timestamp1: 1000000,
            mile2: 1235,
            timestamp2: 1000060,
            speed: 6000,
        };
        let encoded = encode_message(original);
        let decoded = decode_message(encoded.as_ref());
        assert_eq!(
            decoded,
            Message::Ticket {
                plate: msg_str("RE05BKG"),
                road: 368,
                mile1: 1234,
                timestamp1: 1000000,
                mile2: 1235,
                timestamp2: 1000060,
                speed: 6000,
            }
        );
    }

    // === Heartbeat Tests ===
    #[test]
    fn test_want_heartbeat() {
        for &(interval, expected_bytes) in &[
            (10, &[0x40, 0x00, 0x00, 0x00, 0x0a][..]),
            (1243, &[0x40, 0x00, 0x00, 0x04, 0xdb][..]),
        ] {
            let original = Message::WantHeartbeat { interval };
            let encoded = encode_message(original.clone());
            assert_eq!(encoded.as_ref(), expected_bytes);

            let decoded = decode_message(encoded.as_ref());
            assert_eq!(decoded, original);
        }
    }

    #[test]
    fn test_heartbeat() {
        let original = Message::Heartbeat;
        let encoded = encode_message(original.clone());
        assert_eq!(encoded.as_ref(), &[0x41]);

        let decoded = decode_message(encoded.as_ref());
        assert_eq!(decoded, original);
    }

    // === Camera/Dispatcher Tests ===
    #[test]
    fn test_i_am_camera() {
        let test_cases = vec![
            (
                Message::IAmCamera {
                    road: 66,
                    mile: 100,
                    limit: 60,
                },
                &[0x80, 0x00, 0x42, 0x00, 0x64, 0x00, 0x3c][..],
            ),
            (
                Message::IAmCamera {
                    road: 368,
                    mile: 1234,
                    limit: 40,
                },
                &[0x80, 0x01, 0x70, 0x04, 0xd2, 0x00, 0x28][..],
            ),
        ];

        for (original, expected_bytes) in test_cases {
            let encoded = encode_message(original.clone());
            assert_eq!(encoded.as_ref(), expected_bytes);
            let decoded = decode_message(encoded.as_ref());
            assert_eq!(decoded, original);
        }
    }

    #[test]
    fn test_i_am_dispatcher() {
        let test_cases = vec![
            (
                Message::IAmDispatcher {
                    numroads: 1,
                    roads: vec![66],
                },
                &[0x81, 0x01, 0x00, 0x42][..],
            ),
            (
                Message::IAmDispatcher {
                    numroads: 3,
                    roads: vec![66, 368, 5000],
                },
                &[0x81, 0x03, 0x00, 0x42, 0x01, 0x70, 0x13, 0x88][..],
            ),
        ];

        for (original, expected_bytes) in test_cases {
            let encoded = encode_message(original.clone());
            assert_eq!(encoded.as_ref(), expected_bytes);
            let decoded = decode_message(encoded.as_ref());
            assert_eq!(decoded, original);
        }
    }

    // === Partial Data / Streaming Tests ===
    #[test]
    fn test_partial_heartbeat() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from(&[0x41][..]); // Only 1 byte
        assert!(codec.decode(&mut buf).unwrap().is_some()); // Should complete immediately
    }

    #[test]
    fn test_partial_plate_needs_string_content() {
        let mut codec = MessageCodec::new();
        // Has tag + length (4), but no string content
        let mut buf = BytesMut::from(&[0x20, 0x04][..]);
        assert!(codec.decode(&mut buf).unwrap().is_none()); // Need more data
    }

    #[test]
    fn test_partial_plate_needs_timestamp() {
        let mut codec = MessageCodec::new();
        // Has tag + length + "UN1X" (4 chars), but no timestamp
        let mut buf = BytesMut::from(&[0x20, 0x04, b'U', b'N', b'1', b'X'][..]);
        assert!(codec.decode(&mut buf).unwrap().is_none()); // Need more data
    }

    #[test]
    fn test_unknown_tag_error() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::from(&[0x99][..]); // Unknown tag
        let result = codec.decode(&mut buf);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown message tag")
        );
    }

    #[test]
    fn test_internal_message_not_encodable() {
        let mut codec = MessageCodec::new();
        let mut buf = BytesMut::new();

        // Try to encode an internal message
        let internal = Message::Leave {
            client_id: ClientId::new(std::net::SocketAddr::from(([127, 0, 0, 1], 1234))),
        };

        let result = codec.encode(internal, &mut buf);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot encode internal message")
        );
    }
}
