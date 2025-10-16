// https://protohackers.com/problem/6

use super::protocol::*;
use crate::{Error, Result};
use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_util::codec::Framed;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(socket: TcpStream) -> Result<()> {
    let (sink, stream) = Framed::new(socket, MessageCodec::new()).split();
    let _ = handle_client_internal(sink, stream).await;
    Ok(())
}

async fn handle_client_internal<I, O>(mut sink: O, mut stream: I) -> Result<()>
where
    I: Stream<Item = Result<Message>> + Unpin,
    O: Sink<Message, Error = Error> + Unpin,
{
    Ok(())
}
