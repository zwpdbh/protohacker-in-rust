use crate::{Error, Result};

/// Represent possible udp packet received from udp socket.
#[derive(Debug, Clone, PartialEq)]
pub enum UdpMessage {
    Connect {
        session_id: u64,
    },
    Close {
        session_id: u64,
    },
    Data {
        session_id: u64,
        /// refers to the position in the stream of unescaped application-layer bytes，
        /// not the escaped data passed in LRCP.
        pos: u64,
        escaped_data: String,
    },
    Ack {
        session_id: u64,
        length: u64,
    },
}

pub fn parse_packet(buf: &[u8]) -> Result<UdpMessage> {
    let s = std::str::from_utf8(buf).unwrap();
    if !s.starts_with('/') || !s.ends_with('/') {
        return Err(Error::Other("failed to parse_packet".into()));
    }
    let parts: Vec<&str> = s.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return Err(Error::Other("failed to parse_packet".into()));
    }

    let session_id = parts[1]
        .parse::<u64>()
        .map_err(|e| Error::Other(e.to_string()))?;
    if session_id >= 2_147_483_648 {
        return Err(Error::Other("failed to parse_packet".into()));
    }

    match parts[0] {
        "connect" if parts.len() == 2 => Ok(UdpMessage::Connect { session_id }),
        "close" if parts.len() == 2 => Ok(UdpMessage::Close { session_id }),
        "ack" if parts.len() == 3 => {
            let length = parts[2].parse().unwrap();
            Ok(UdpMessage::Ack { session_id, length })
        }
        "data" if parts.len() == 4 => {
            let pos = parts[2].parse().unwrap();
            Ok(UdpMessage::Data {
                session_id,
                pos,
                escaped_data: parts[3].to_string(),
            })
        }
        _ => return Err(Error::Other("failed to parse_packet".into())),
    }
}

pub fn escape_data(s: &str) -> String {
    s.replace("\\", "\\\\").replace("/", "\\/")
}

pub fn unescape_data(s: &str) -> String {
    s.replace("\\/", "/").replace("\\\\", "\\")
}

#[cfg(test)]
mod protocol_parser_tests {
    #![allow(unused)]
    use super::*;

    #[test]
    fn test_parse_connect_valid() {
        let input = b"/connect/12345/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(packet, UdpMessage::Connect { session_id: 12345 });
    }

    #[test]
    fn test_parse_connect_invalid_too_many_fields() {
        let input = b"/connect/12345/extra/";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_connect_invalid_missing_slash() {
        let input = b"connect/12345/";
        assert!(parse_packet(input).is_err());

        let input = b"/connect/12345";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_close_valid() {
        let input = b"/close/999/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(packet, UdpMessage::Close { session_id: 999 });
    }

    #[test]
    fn test_parse_ack_valid() {
        let input = b"/ack/42/100/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            UdpMessage::Ack {
                session_id: 42,
                length: 100
            }
        );
    }

    #[test]
    fn test_parse_ack_invalid_field_count() {
        let input = b"/ack/42/";
        assert!(parse_packet(input).is_err());

        let input = b"/ack/42/100/extra/";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_data_valid() {
        let input = b"/data/1/0/hello/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            UdpMessage::Data {
                session_id: 1,
                pos: 0,
                escaped_data: "hello".to_string()
            }
        );
    }

    #[test]
    fn test_parse_data_with_escaped_slash() {
        let input = b"/data/1/0/\\/"; // represents a single "/"
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            UdpMessage::Data {
                session_id: 1,
                pos: 0,
                escaped_data: r#"\"#.to_string() // raw backslash + forward slash
            }
        );
    }

    #[test]
    fn test_parse_data_invalid_field_count() {
        let input = b"/data/1/0/";
        assert!(parse_packet(input).is_err());

        let input = b"/data/1/0/hello/extra/";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_invalid_session_id_too_large() {
        let input = b"/connect/2147483648/"; // 2^31, which is not allowed
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_invalid_session_id_negative() {
        // Negative numbers aren't representable in u64 via parse, but malformed string
        let input = b"/connect/-123/";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let input = b"/unknown/123/";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_parse_empty_packet() {
        let input = b"//";
        assert!(parse_packet(input).is_err());
    }

    #[test]
    fn test_escape_data() {
        assert_eq!(escape_data("hello"), "hello");
        assert_eq!(escape_data("a/b"), r"a\/b");
        assert_eq!(escape_data(r"a\b"), r"a\\b");
        assert_eq!(escape_data("/\\"), r"\/\\");
    }

    #[test]
    fn test_unescape_data() {
        assert_eq!(unescape_data("hello"), "hello");
        assert_eq!(unescape_data(r"a\/b"), "a/b");
        assert_eq!(unescape_data(r"a\\b"), r"a\b");
        assert_eq!(unescape_data(r"a\\/b\\\\"), r"a\/b\\");
        assert_eq!(unescape_data(r"\/\\"), "/\\");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        let original = "This has / and \\ and even \\/ and \\\\";
        let escaped = escape_data(original);
        let unescaped = unescape_data(&escaped);
        assert_eq!(original, unescaped);
    }
}
