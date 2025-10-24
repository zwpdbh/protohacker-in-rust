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
    let (listener, mut accept_rx) = LrcpListener::bind(&address).await?;

    while let Some((stream, peer_addr)) = accept_rx.recv().await {
        // Spawn a task to handle this session
        tokio::spawn(async move {
            if let Err(e) = handle_session(stream, peer_addr).await {
                eprintln!("Session error ({}): {}", peer_addr, e);
            }
        });
    }

    Ok(())
}

async fn handle_session(mut stream: LrcpStream, _peer_addr: SocketAddr) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            // EOF — client closed gracefully
            break;
        }

        // Remove trailing \n (read_line keeps it)
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }

        // Reverse the line and send back with \n
        let reversed: String = line.chars().rev().collect();
        let response = format!("{}\n", reversed);

        // Write response
        stream.write_all(response.as_bytes()).await?;
    }

    Ok(())
}
