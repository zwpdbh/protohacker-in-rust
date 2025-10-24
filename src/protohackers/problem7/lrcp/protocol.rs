#![allow(unused)]
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct LrcpPacket {
    pub session_id: u64,
    pub kind: LrcpPacketKind,
}

#[derive(Debug, Clone)]
pub enum LrcpPacketKind {
    Connect,
    Close,
    Data { pos: u64, escaped_data: String },
    Ack { length: u64 },
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
        "connect" if parts.len() == 2 => Some(LrcpPacket {
            session_id,
            kind: LrcpPacketKind::Connect,
        }),
        "close" if parts.len() == 2 => Some(LrcpPacket {
            session_id,
            kind: LrcpPacketKind::Close,
        }),
        "ack" if parts.len() == 3 => {
            let length = parts[2].parse().ok()?;
            Some(LrcpPacket {
                session_id,
                kind: LrcpPacketKind::Ack { length },
            })
        }
        "data" if parts.len() == 4 => {
            let pos = parts[2].parse().ok()?;
            Some(LrcpPacket {
                session_id,
                kind: LrcpPacketKind::Data {
                    pos,
                    escaped_data: parts[3].to_string(),
                },
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
