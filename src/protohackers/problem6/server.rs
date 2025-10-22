// https://protohackers.com/problem/6

use super::client::*;
use super::state::*;
use crate::Result;
use tokio::net::TcpListener;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address.clone()).await?;
    let state_tx = StateTx::new();

    loop {
        let (socket, addr) = listener.accept().await?;
        let client_id = ClientId::new(addr);
        tokio::spawn(handle_client(client_id, state_tx.clone(), socket));
    }
}
