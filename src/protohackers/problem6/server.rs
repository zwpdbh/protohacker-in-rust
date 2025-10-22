// https://protohackers.com/problem/6

use super::client::*;
use super::state::*;
use crate::Result;
use crate::protohackers::HOST;
use tokio::net::TcpListener;
use tracing::info;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("{HOST}:{port}");
    let listener = TcpListener::bind(address.clone()).await?;
    info!("problem6 listen on: {}", address);
    let state_tx = StateTx::new();

    loop {
        let (socket, addr) = listener.accept().await?;
        let client_id = ClientId::new(addr);
        tokio::spawn(handle_client(client_id, state_tx.clone(), socket));
    }
}
