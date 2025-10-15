// Unlike TCP stream, UDP is message-oriented which means we don't need Decoder/Encoder codecs like in TCP
// There is no stream to frame.

// Parse raw UDP datagram → Request
#[derive(Debug, PartialEq)]
pub enum Request {
    Insert { key: String, value: String },
    Retrieve { key: String },
}

impl Request {
    pub fn parse(payload: &[u8]) -> Option<Request> {
        let s = std::str::from_utf8(payload).ok()?;

        if let Some(eq_idx) = s.find('=') {
            let key = s[..eq_idx].to_string();
            let value = s[eq_idx + 1..].to_string();

            if key == "version" {
                None // ignore inserts to "version"
            } else {
                Some(Request::Insert { key, value })
            }
        } else {
            Some(Request::Retrieve { key: s.to_string() })
        }
    }
}

// Format response for a given key + value
pub fn format_response(key: &str, value: &str) -> Vec<u8> {
    format!("{}={}", key, value).into_bytes()
}
