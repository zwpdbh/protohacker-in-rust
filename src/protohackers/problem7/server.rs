#![allow(unused)]
use super::client::*;
use super::lrcp::*;
use crate::Result;
use crate::protohackers::HOST;
use std::net::SocketAddr;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("{}:{}", HOST, port);
    let mut listener = LrcpListener::bind(&address).await?;

    loop {
        match listener.accept().await {
            Some((stream, peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_session(stream, peer_addr).await {
                        eprintln!("Session error ({}): {}", peer_addr, e);
                    }
                });
            }
            None => break,
        }
    }

    Ok(())
}

async fn handle_session(mut stream: LrcpStream, _peer_addr: SocketAddr) -> Result<()> {
    let mut buffered_stream = BufReader::new(stream); // ← takes ownership
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = buffered_stream.read_line(&mut line).await?;

        if bytes_read == 0 {
            break; // EOF
        }

        // Strip \n (and \r if present)
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        let reversed: String = line.chars().rev().collect();
        let response = format!("{}\n", reversed);

        // Write through the buffered stream
        buffered_stream.write_all(response.as_bytes()).await?;
    }

    Ok(())
}
