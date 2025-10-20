#![allow(unused)]

use super::client::*;
use super::protocol::*;
use crate::{Error, Result};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct State {
    sender: mpsc::UnboundedSender<Message>,
}

pub struct StateHandle {
    receiver: mpsc::UnboundedReceiver<Message>,
}

impl StateHandle {
    async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
}

impl State {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(run_state(StateHandle { receiver: rx }));
        State { sender: tx }
    }

    // A client will Join the State and return a ClientHandle
    pub fn join(&self, client_id: ClientId) -> Result<ClientHandle> {
        let (client_tx, client_rx) = mpsc::unbounded_channel::<Message>();

        let _ = self
            .sender
            .send(Message::Join {
                client: Client {
                    client_id: client_id.clone(),
                    sender: client_tx,
                    role: ClientRole::Undefined,
                },
            })
            .map_err(|e| Error::General(e.to_string()))?;

        return Ok(ClientHandle {
            client_id,
            receiver: client_rx,
        });
    }

    pub fn send(&self, msg: Message) -> Result<()> {
        let _ = self
            .sender
            .send(msg)
            .map_err(|e| Error::General(e.to_string()));
        Ok(())
    }

    pub fn leave(&self, client_id: ClientId) -> Result<()> {
        self.sender
            .send(Message::Leave { client_id })
            .map_err(|_| Error::General("State channel closed".into()))
    }
}

#[derive(Debug, Clone)]
struct Camera {
    road: u16,
    mile: u16,
    limit: u16,
}

#[derive(Debug, Clone)]
struct Plate {
    plate: String,
    timestamp: u32,
}

#[derive(Debug, Clone)]
struct PlateEvent {
    camera: Camera,
    plate: Plate,
}

async fn run_state(mut state_handle: StateHandle) -> Result<()> {
    // initalize state
    // loop receive message from handle

    let mut clients: HashMap<ClientId, Client> = HashMap::new();

    while let Some(msg) = state_handle.recv().await {
        match msg {
            Message::Join { client } => {
                let _ = clients.insert(client.client_id.clone(), client);
            }
            _ => {
                todo!()
            }
        }
    }

    Ok(())
}
