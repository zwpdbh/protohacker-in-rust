use super::protocol::*;
use crate::Result;

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
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
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

        // Parse request
        if let Some(req) = Request::parse(payload) {
            match req {
                Request::Insert { key, value } => {
                    // Update store
                    db.insert(key, value);
                    // No response for inserts
                }
                Request::Retrieve { key } => {
                    if let Some(value) = db.retrieve(&key) {
                        let response = format_response(&key, &value);
                        socket.send_to(&response, src_addr).await?;
                    } else {
                        // Option: send "key=" or do nothing.
                        // Let's send "key=" for clarity.
                        let response = format!("{}=", key).into_bytes();
                        socket.send_to(&response, src_addr).await?;
                    }
                }
            }
        }
        // If parse fails (e.g., invalid UTF-8), ignore silently (UDP best-effort)
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;

    #[tokio::test]
    async fn case01() -> Result<()> {
        Ok(())
    }
}
