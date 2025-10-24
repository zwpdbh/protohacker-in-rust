#![allow(unused)]
use crate::Result;

use super::client::*;
// use super::lrcp::LrcpListener;
use crate::protohackers::HOST;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn run(port: u32) -> Result<()> {
    let address = format!("127.0.0.1:{}", port);
    // let listener = LrcpListener::bind(address.clone()).await?;

    // loop {
    //     let (socket, addr) = listener.accept().await?;
    //     let client_id = ClientId::new(addr);
    //     tokio::spawn(handle_client(client_id, socket));
    // }
    todo!()
}
