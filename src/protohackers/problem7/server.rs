use super::lrcp::*;
use crate::Result;
use crate::protohackers::HOST;
use std::net::SocketAddr;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tracing::debug;
use tracing::error;
use tracing::{Level, span};

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
    let span = span!(Level::INFO, "handle_session");
    let _enter = span.enter();

    let mut buffered_stream = BufReader::new(stream);
    let mut line = String::new();

    while buffered_stream.read_line(&mut line).await? > 0 {
        let reversed = line.trim_end().chars().rev().collect::<String>();
        debug!("reversed : {}", reversed);
        buffered_stream
            .write_all(format!("{}\n", reversed).as_bytes())
            .await?;
    }

    Ok(())
}
