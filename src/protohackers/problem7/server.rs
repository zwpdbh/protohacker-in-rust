use super::lrcp::*;
use crate::Result;
use crate::protohackers::HOST;
use std::net::SocketAddr;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::debug;
use tracing::error;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("{}:{}", HOST, port);
    let mut listener = LrcpListener::bind(&address).await?;

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_session(stream, peer_addr).await {
                error!("Session error ({}): {}", peer_addr, e);
            }
        });
    }
}

async fn handle_session(stream: LrcpStream, _peer_addr: SocketAddr) -> Result<()> {
    let mut buffered_stream = BufReader::new(stream);
    let mut line = String::new();

    loop {
        let bytes_read = buffered_stream.read_line(&mut line).await?;

        if bytes_read == 0 {
            debug!("EOF reached");
            break;
        }

        let reversed: String = line.chars().rev().collect();
        debug!("reversed: {}", reversed);

        let response: String = reversed.trim().to_string() + "\n";
        if let Err(e) = buffered_stream.write_all(response.as_bytes()).await {
            error!("Write failed: {}", e);
            break;
        }
        line.clear();
    }

    Ok(())
}
