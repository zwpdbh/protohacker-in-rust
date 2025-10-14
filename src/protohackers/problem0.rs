use crate::Result;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::info;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;

    info!("echo server listening on {address}");
    loop {
        let (socket, _addr) = listener.accept().await?;

        tokio::spawn(handle_client(socket));
    }
}

pub async fn handle_client(mut socket: TcpStream) -> Result<()> {
    let mut buf = [0; 1024];
    loop {
        match socket.read(&mut buf).await {
            Ok(0) => return Ok(()),
            Ok(n) => {
                socket.write_all(&buf[..n]).await?;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    // --- Test Helper: Start server on random port ---
    async fn start_test_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn server in background
        tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                tokio::spawn(handle_client(socket));
            }
        });

        addr
    }

    // --- Client Helper ---
    async fn echo_client(addr: SocketAddr, msg: &str) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(msg.as_bytes()).await.unwrap();
        stream.shutdown().await.unwrap(); // Signal EOF after send

        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    }

    #[tokio::test]
    async fn test_echo_server_with_multiple_clients() {
        let addr = start_test_server().await;

        // Spawn 3 clients
        let mut handles = Vec::new();
        for i in 0..3 {
            let addr = addr;
            let handle = tokio::spawn(async move {
                let msg = format!("hello from client {}", i);
                let response = echo_client(addr, &msg).await;
                (i, msg, response)
            });
            handles.push(handle);
        }

        // Await all and assert
        for handle in handles {
            let (i, original, echoed) = handle.await.unwrap();
            assert_eq!(
                echoed, original,
                "Client {} did not receive correct echo",
                i
            );
        }
    }
}
