use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

use crate::{Error, Result};

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(address.clone()).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<()> {
    let (input_stream, output_stream) = socket.split();
    let _ = handle_client_internal(input_stream, output_stream).await;
    Ok(())
}

async fn handle_client_internal(
    input_stream: impl AsyncRead + Unpin,
    mut output_stream: impl AsyncWrite + Unpin,
) -> Result<()> {
    Ok(())
}
