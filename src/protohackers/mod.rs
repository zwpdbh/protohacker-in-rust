pub mod problem0;
pub mod problem1;
pub mod problem2;
pub mod problem3;
pub mod problem4;
pub mod problem5;

use crate::Result;
use std::{future::Future, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

pub async fn run_server<H, F>(port: u32, handler: H) -> Result<()>
where
    H: Fn(TcpStream) -> F,
    F: Future<Output = Result<()>> + Send + 'static,
{
    run_server_with_state(port, (), |_, stream, _| handler(stream)).await
}

pub async fn run_server_with_state<H, S, F>(port: u32, state: S, handler: H) -> Result<()>
where
    S: Clone,
    H: Fn(S, TcpStream, SocketAddr) -> F,
    F: Future<Output = Result<()>> + Send + 'static,
{
    let listener = TcpListener::bind(&format!("127.0.0.1:{}", port)).await?;

    info!("Starting server at 127.0.0.1:{}", port);
    loop {
        let (socket, address) = listener.accept().await?;

        debug!("Got connection from {}", address);
        let future = handler(state.clone(), socket, address);
        tokio::task::spawn(async move {
            if let Err(err) = future.await {
                error!("Error handling connection {}: {}", address, err);
            }
        });
    }
}
