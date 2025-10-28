#![allow(unused)]
use std::net::SocketAddr;

/// Lrcp protocol message
/// represent valid packet received from UDP socket
#[derive(Debug, Clone)]
pub enum LrcpPacket {
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

pub fn parse_packet(buf: &[u8]) -> Option<LrcpPacket> {
    let s = std::str::from_utf8(buf).ok()?;
    if !s.starts_with('/') || !s.ends_with('/') {
        return None;
    }
    let parts: Vec<&str> = s.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }

    let session_id = parts[1].parse().ok()?;
    if session_id >= 2_147_483_648 {
        return None;
    }

    match parts[0] {
        "connect" if parts.len() == 2 => Some(LrcpPacket::Connect { session_id }),
        "close" if parts.len() == 2 => Some(LrcpPacket::Close { session_id }),
        "ack" if parts.len() == 3 => {
            let length = parts[2].parse().ok()?;
            Some(LrcpPacket::Ack { session_id, length })
        }
        "data" if parts.len() == 4 => {
            let pos = parts[2].parse().ok()?;
            Some(LrcpPacket::Data {
                session_id,
                pos,
                escaped_data: parts[3].to_string(),
            })
        }
        _ => None,
    }
}

pub fn escape_data(s: &str) -> String {
    s.replace("\\", "\\\\").replace("/", "\\/")
}

pub fn unescape_data(s: &str) -> String {
    s.replace("\\/", "/").replace("\\\\", "\\")
}
