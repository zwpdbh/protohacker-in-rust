use super::protocol::*;
use crate::Result;

use crate::protohackers::HOST;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;

struct Db {
    store: Arc<Mutex<HashMap<String, String>>>,
}

impl Db {
    fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn insert(&mut self, k: String, v: String) -> Option<String> {
        let mut s = self.store.lock().unwrap();
        s.insert(k, v)
    }

    fn retrieve(&self, k: &str) -> Option<String> {
        let s = self.store.lock().unwrap();
        s.get(k).cloned()
    }
}

pub async fn run(port: u32) -> Result<()> {
    let addr: SocketAddr = format!("{HOST}:{port}").parse().unwrap();
    let socket = UdpSocket::bind(addr).await?;

    let mut db = Db::new();
    let _ = db.insert(
        "version".to_string(),
        "Ken's Key-Value Store 1.0".to_string(),
    );

    let mut buf = vec![0u8; 65536];
    loop {
        let (len, src_addr) = socket.recv_from(&mut buf).await?;
        let payload = &buf[..len];

        if let Some(resonse) = handle_message(&mut db, payload) {
            socket.send_to(&resonse, src_addr).await?;
        }
    }
}

fn handle_message(db: &mut Db, payload: &[u8]) -> Option<Vec<u8>> {
    // Parse request
    if let Some(req) = Request::parse(payload) {
        match req {
            Request::Insert { key, value } => {
                // Update store
                db.insert(key, value);
                return None;
            }
            Request::Retrieve { key } => {
                if let Some(value) = db.retrieve(&key) {
                    let response = format_response(&key, &value);
                    return Some(response);
                } else {
                    // Option: send "key=" or do nothing.
                    // Let's send "key=" for clarity.
                    let response = format!("{}=", key).into_bytes();
                    return Some(response);
                }
            }
        }
    } else {
        // If parse fails (e.g., invalid UTF-8), ignore silently (UDP best-effort)
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;

    #[test]
    fn test_parse_insert_simple() {
        let req = Request::parse(b"foo=bar").unwrap();
        assert_eq!(
            req,
            Request::Insert {
                key: "foo".to_string(),
                value: "bar".to_string()
            }
        );
    }

    #[test]
    fn test_parse_insert_with_equals_in_value() {
        let req = Request::parse(b"foo=bar=baz").unwrap();
        assert_eq!(
            req,
            Request::Insert {
                key: "foo".to_string(),
                value: "bar=baz".to_string()
            }
        );
    }

    #[test]
    fn test_parse_insert_empty_value() {
        let req = Request::parse(b"foo=").unwrap();
        assert_eq!(
            req,
            Request::Insert {
                key: "foo".to_string(),
                value: "".to_string()
            }
        );
    }

    #[test]
    fn test_parse_insert_multiple_equals() {
        let req = Request::parse(b"foo===").unwrap();
        assert_eq!(
            req,
            Request::Insert {
                key: "foo".to_string(),
                value: "==".to_string()
            }
        );
    }

    #[test]
    fn test_parse_insert_empty_key() {
        let req = Request::parse(b"=foo").unwrap();
        assert_eq!(
            req,
            Request::Insert {
                key: "".to_string(),
                value: "foo".to_string()
            }
        );
    }

    #[test]
    fn test_parse_retrieve() {
        let req = Request::parse(b"message").unwrap();
        assert_eq!(
            req,
            Request::Retrieve {
                key: "message".to_string()
            }
        );
    }

    #[test]
    fn test_parse_retrieve_empty_key() {
        let req = Request::parse(b"").unwrap();
        assert_eq!(
            req,
            Request::Retrieve {
                key: "".to_string()
            }
        );
    }

    #[test]
    fn test_parse_ignore_version_insert() {
        // Should return None → ignored
        assert!(Request::parse(b"version=hacked").is_none());
    }

    #[test]
    fn test_parse_version_retrieve() {
        let req = Request::parse(b"version").unwrap();
        assert_eq!(
            req,
            Request::Retrieve {
                key: "version".to_string()
            }
        );
    }

    #[test]
    fn test_parse_non_utf8() {
        // Invalid UTF-8 → ignored
        assert!(Request::parse(&[0xFF, 0xFE]).is_none());
    }

    #[test]
    fn test_format_response() {
        assert_eq!(format_response("key", "value"), b"key=value".to_vec());
        assert_eq!(format_response("", "hello"), b"=hello".to_vec());
        assert_eq!(format_response("empty", ""), b"empty=".to_vec());
    }

    // Now test the full handle_message logic with a mock DB
    #[test]
    fn test_handle_insert_and_retrieve() {
        let mut db = Db::new();
        // Pre-populate version
        db.insert(
            "version".to_string(),
            "Ken's Key-Value Store 1.0".to_string(),
        );

        // Insert
        let resp = handle_message(&mut db, b"foo=bar");
        assert!(resp.is_none());

        // Retrieve
        let resp = handle_message(&mut db, b"foo").unwrap();
        assert_eq!(resp, b"foo=bar");

        // Retrieve missing key → returns "key="
        let resp = handle_message(&mut db, b"missing").unwrap();
        assert_eq!(resp, b"missing=");
    }

    #[test]
    fn test_handle_version_retrieve() {
        let mut db = Db::new();
        db.insert(
            "version".to_string(),
            "Ken's Key-Value Store 1.0".to_string(),
        );

        let resp = handle_message(&mut db, b"version").unwrap();
        assert_eq!(resp, b"version=Ken's Key-Value Store 1.0");
    }

    #[test]
    fn test_handle_version_insert_ignored() {
        let mut db = Db::new();
        db.insert(
            "version".to_string(),
            "Ken's Key-Value Store 1.0".to_string(),
        );

        // Try to overwrite version
        let resp = handle_message(&mut db, b"version=hacked");
        assert!(resp.is_none());

        // Version should still be original
        let resp = handle_message(&mut db, b"version").unwrap();
        assert_eq!(resp, b"version=Ken's Key-Value Store 1.0");
    }

    #[test]
    fn test_handle_empty_key() {
        let mut db = Db::new();
        db.insert(
            "version".to_string(),
            "Ken's Key-Value Store 1.0".to_string(),
        );

        // Insert with empty key
        let resp = handle_message(&mut db, b"=hello");
        assert!(resp.is_none());

        // Retrieve empty key
        let resp = handle_message(&mut db, b"").unwrap();
        assert_eq!(resp, b"=hello");
    }

    #[test]
    fn test_handle_update_existing_key() {
        let mut db = Db::new();
        db.insert(
            "version".to_string(),
            "Ken's Key-Value Store 1.0".to_string(),
        );

        handle_message(&mut db, b"key=old");
        handle_message(&mut db, b"key=new");

        let resp = handle_message(&mut db, b"key").unwrap();
        assert_eq!(resp, b"key=new");
    }
}
