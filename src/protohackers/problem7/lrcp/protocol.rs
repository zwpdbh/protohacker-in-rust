use crate::{Error, Result};

/// Represent possible udp packet received from udp socket.
#[derive(Debug, Clone, PartialEq)]
pub enum LrcpMessage {
    Connect {
        session_id: u64,
    },
    ClientClose {
        session_id: u64,
    },
    SessionTerminate {
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

const MAX_INT: u64 = 2_147_483_648; // 2^31

fn parse_int(s: &str) -> Result<u64> {
    s.parse::<u64>()
        .map_err(|_| Error::Other("invalid integer".into()))
        .and_then(|n| {
            if n < MAX_INT {
                Ok(n)
            } else {
                Err(Error::Other("integer too large".into()))
            }
        })
}

/// Tokenize the string after the leading '/', respecting `\/` and `\\` escapes.
/// Returns a Vec of tokens, where `/` is delimiter, but `\/` is literal '/'.
fn tokenize(s: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut acc = String::new();
    let mut chars = s.char_indices().peekable();
    let last_index = s.len() - 1;
    while let Some((_index, ch)) = chars.next() {
        match ch {
            '\\' => {
                // Look ahead to next char
                if let Some((i, next)) = chars.peek() {
                    match *next {
                        '/' if *i != last_index => {
                            // Consume the next char
                            // let _ = chars.next().unwrap();
                            // treat "\/" as whole normal data
                            acc.push('\\');
                            acc.push(*next);
                            chars.next();
                        }
                        '/' if *i == last_index => {
                            // Consume the next char
                            // let _ = chars.next().unwrap();
                            // treat "\/" as whole normal data
                            acc.push('\\');
                        }
                        '\\' => {
                            // Consume the next char
                            // let _ = chars.next().unwrap();
                            // treat "\/" as whole normal data
                            acc.push('\\');
                            acc.push(*next);
                            chars.next();
                        }
                        _ => {
                            // Lone backslash → treat as literal? But Elixir doesn't allow this.
                            // In Elixir, unknown escapes would just pass through?
                            // However, in your Elixir code, only "\\/" is handled explicitly.
                            // Others like "\\x" would be processed as "\\x" → but your Elixir
                            // just appends char by char, so "\\" followed by 'x' becomes "\\x".
                            // So we must preserve unknown escapes too.
                            // acc.push('\\');
                            acc.push(*next);
                            chars.next(); // consume the next
                        }
                    }
                } else {
                    // Trailing backslash — invalid? Elixir would just append it.
                    // acc.push('\\');
                    return Err(Error::Other("ill-format when parse '\\'".to_string()));
                }
            }
            '/' => {
                // Unescaped slash: end current token
                tokens.push(acc);
                acc = String::new();
            }
            _ => {
                acc.push(ch);
            }
        }
    }

    // Push the last token (even if empty)
    tokens.push(acc);
    Ok(tokens)
}

pub fn parse_packet(buf: &[u8]) -> Result<LrcpMessage> {
    let s = std::str::from_utf8(buf).map_err(|_| Error::Other("invalid UTF-8".into()))?;

    if !(s.starts_with('/') && s.ends_with('/')) {
        return Err(Error::Other("packet must start and end with '/'".into()));
    }

    // Skip the leading '/'
    let rest = &s[1..];

    let parts = tokenize(rest)?;
    let parts: Vec<&str> = parts
        .iter()
        .map(|s| s.as_str())
        .filter(|s| *s != "")
        .collect();

    // debug!("parsed parts: {:?}", parts);

    // Now dispatch by first field
    match parts.as_slice() {
        ["connect", session_str] => {
            let session_id = parse_int(session_str)?;
            Ok(LrcpMessage::Connect { session_id })
        }
        ["close", session_str] => {
            let session_id = parse_int(session_str)?;
            Ok(LrcpMessage::ClientClose { session_id })
        }
        ["ack", session_str, pos_str] => {
            let session_id = parse_int(session_str)?;
            let length = parse_int(pos_str)?;
            Ok(LrcpMessage::Ack { session_id, length })
        }
        ["data", session_str, pos_str, data] => {
            let session_id = parse_int(session_str)?;
            let pos = parse_int(pos_str)?;
            Ok(LrcpMessage::Data {
                session_id,
                pos,
                escaped_data: data.to_string(),
            })
        }
        _ => Err(Error::Other("invalid packet format".into())),
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
    use crate::tracer;
    use std::sync::Once;

    static TRACING: Once = Once::new();

    fn init_tracing() {
        TRACING.call_once(|| {
            let _x = tracer::setup_simple_tracing();
        });
    }

    #[test]
    fn test_parse_connect_valid() {
        let input = b"/connect/12345/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(packet, LrcpMessage::Connect { session_id: 12345 });
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
        assert_eq!(packet, LrcpMessage::ClientClose { session_id: 999 });
    }

    #[test]
    fn test_parse_ack_valid() {
        let input = b"/ack/42/100/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            LrcpMessage::Ack {
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
    fn test_parse_data_valid_a() {
        let input = b"/data/1/0/hello/";
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            LrcpMessage::Data {
                session_id: 1,
                pos: 0,
                escaped_data: "hello".to_string()
            }
        );
    }

    #[test]
    fn test_parse_data_valid_b() {
        let _ = init_tracing();
        let input = "/data/123/456/hello\\/world\\\\!\n/";
        let packet = parse_packet(input.as_bytes()).unwrap();
        assert_eq!(
            packet,
            LrcpMessage::Data {
                session_id: 123,
                pos: 456,
                escaped_data: "hello\\/world\\\\!\n".to_string()
            }
        );
    }

    #[test]
    fn test_parse_data_with_escaped_slash() {
        let input = b"/data/1/0/\\/"; // represents a single "\"
        let packet = parse_packet(input).unwrap();
        assert_eq!(
            packet,
            LrcpMessage::Data {
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
