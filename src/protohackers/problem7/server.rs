use super::lrcp::*;
use crate::Result;
use crate::protohackers::HOST;
use std::net::SocketAddr;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;

use tracing::error;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("{}:{}", HOST, port);
    let mut listener = LrcpListener::bind(&address).await?;

    loop {
        match listener.accept().await {
            Some((stream, peer_addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = handle_session(stream, peer_addr).await {
                        error!("Session error ({}): {}", peer_addr, e);
                    }
                });
            }
            None => break,
        }
    }

    Ok(())
}

async fn handle_session(stream: LrcpStream, _peer_addr: SocketAddr) -> Result<()> {
    let mut buffered_stream = BufReader::new(stream);
    let mut line = String::new();

    while buffered_stream.read_line(&mut line).await? > 0 {
        let reversed = line.trim_end().chars().rev().collect::<String>();
        buffered_stream
            .write_all(format!("{}\n", reversed).as_bytes())
            .await?;
    }

    Ok(())
}
