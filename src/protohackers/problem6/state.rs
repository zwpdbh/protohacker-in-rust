#![allow(unused)]

use super::client::*;
use super::protocol::*;
use crate::{Error, Result};
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::event;

#[derive(Debug, Clone)]
pub struct StateTx {
    sender: mpsc::UnboundedSender<Message>,
}

pub struct StateChannel {
    sender: mpsc::UnboundedSender<Message>,
    receiver: mpsc::UnboundedReceiver<Message>,
}

impl StateChannel {
    async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
    }
    async fn send(&mut self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|e| Error::General(e.to_string()))
    }
}

impl StateTx {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(run_state(StateChannel {
            receiver: rx,
            sender: tx.clone(),
        }));
        StateTx { sender: tx }
    }

    // A client will Join the State and return a ClientHandle
    pub fn join(&self, client_id: ClientId) -> Result<ClientChannel> {
        let (client_tx, client_rx) = mpsc::unbounded_channel::<Message>();

        let _ = self
            .sender
            .send(Message::Join {
                client: Client {
                    client_id: client_id.clone(),
                    sender: client_tx.clone(),
                    role: ClientRole::Undefined,
                },
            })
            .map_err(|e| Error::General(e.to_string()))?;

        return Ok(ClientChannel {
            client_id,
            sender: client_tx,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Plate(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RoadInfo {
    road: u16,
    limit: u16,
}
struct Limit(u16);
struct Mile(u16);
struct Timestamp(u32);

struct PlateEvents {
    events: Vec<(Mile, Timestamp)>,
}

impl PlateEvents {
    fn new() -> Self {
        PlateEvents { events: vec![] }
    }
}

struct PlateTracker {
    plate_events: HashMap<Plate, PlateEvents>,
}

impl PlateTracker {
    fn new() -> Self {
        PlateTracker {
            plate_events: HashMap::new(),
        }
    }
}

struct TicketManager {
    roads: HashMap<RoadInfo, PlateTracker>,
    ticketed: HashSet<(Plate, u32)>,
    pending_tickets: Vec<Ticket>,
    state_channel_sender: mpsc::UnboundedSender<Message>,
}

struct Ticket {
    plate: String,
    road: u16,
    mile1: u16,
    timestamp1: u32,
    mile2: u16,
    timestamp2: u32,
    speed: u16,
}

impl TicketManager {
    fn new(state_channel_sender: mpsc::UnboundedSender<Message>) -> Self {
        TicketManager {
            roads: HashMap::new(),
            ticketed: HashSet::new(),
            pending_tickets: vec![],
            state_channel_sender,
        }
    }

    fn add_ticket(&mut self, ticket: Ticket) {
        self.pending_tickets.push(ticket);
    }

    async fn flush_pending_tickets(&mut self, clients: &HashMap<ClientId, Client>) {
        let tickets = std::mem::take(&mut self.pending_tickets);
        if tickets.is_empty() {
            return;
        }

        // Build road → dispatcher map (store reference, no clone!)
        let mut road_to_dispatcher: HashMap<u16, &Client> = HashMap::new();
        for client in clients.values() {
            if let ClientRole::Dispatcher { roads } = &client.role {
                for &road in roads {
                    road_to_dispatcher.entry(road).or_insert(client);
                }
            }
        }

        let mut tickets_to_keep = Vec::new();

        // Dispatch tickets (move ticket, no clone)
        for ticket in tickets {
            if let Some(client) = road_to_dispatcher.get(&ticket.road) {
                let _ = client.sender.send(Message::Ticket {
                    plate: ticket.plate.into(),
                    road: ticket.road,
                    mile1: ticket.mile1,
                    timestamp1: ticket.timestamp1,
                    mile2: ticket.mile2,
                    timestamp2: ticket.timestamp2,
                    speed: ticket.speed,
                });
            } else {
                tickets_to_keep.push(ticket);
            }
        }

        self.pending_tickets = tickets_to_keep
    }

    // add a new plate event and generate a Option<Ticket>
    fn add_plate_event(
        &mut self,
        road: &u16,
        mile: &u16,
        limit: &u16,
        plate: &str,
        timestamp: &u32,
    ) -> Option<Ticket> {
        let road_info = RoadInfo {
            road: *road,
            limit: *limit,
        };
        let plate_key = Plate(plate.to_string());
        let mile_val = Mile(*mile);
        let ts_val = Timestamp(*timestamp);

        // Get or create PlateTracker for this road
        let tracker = self
            .roads
            .entry(road_info.clone())
            .or_insert_with(PlateTracker::new);

        // Get or create PlateEvents for this plate
        let events = tracker
            .plate_events
            .entry(plate_key.clone())
            .or_insert_with(PlateEvents::new);

        // Add new event
        events.events.push((mile_val, ts_val));
        let new_index = events.events.len() - 1;

        // Compare with all previous events
        for i in 0..new_index {
            let (m1, t1) = &events.events[i];
            let (m2, t2) = &events.events[new_index];

            if t1.0 == t2.0 {
                continue; // avoid division by zero
            }

            // Order by time
            let (earlier_mile, earlier_ts, later_mile, later_ts) = if t1.0 < t2.0 {
                (m1.0, t1.0, m2.0, t2.0)
            } else {
                (m2.0, t2.0, m1.0, t1.0)
            };

            let delta_time = later_ts - earlier_ts;
            let delta_mile = if later_mile > earlier_mile {
                later_mile - earlier_mile
            } else {
                earlier_mile - later_mile
            };

            let speed_100x = compute_speed_100x(delta_mile, delta_time);
            let threshold = road_info.limit * 100 + 50;

            if speed_100x < threshold {
                continue;
            }

            // Check daily ticket limit
            let day1 = day_from_timestamp(earlier_ts);
            let day2 = day_from_timestamp(later_ts);

            let violates_limit =
                (day1..=day2).any(|day| self.ticketed.contains(&(plate_key.clone(), day)));

            if violates_limit {
                continue;
            }

            // Mark all days in range as ticketed
            for day in day1..=day2 {
                self.ticketed.insert((plate_key.clone(), day));
            }

            // Return ticket
            return Some(Ticket {
                plate: plate.to_string(),
                road: road_info.road,
                mile1: earlier_mile,
                timestamp1: earlier_ts,
                mile2: later_mile,
                timestamp2: later_ts,
                speed: speed_100x,
            });
        }

        None
    }
}

async fn run_state(mut state_channel: StateChannel) -> Result<()> {
    // initalize state
    let mut clients: HashMap<ClientId, Client> = HashMap::new();
    let mut ticket_manager = TicketManager::new(state_channel.sender.clone());
    // let mut pending_tickets: Vec<Ticket> = Vec::new();

    // loop receive message from handle
    while let Some(msg) = state_channel.recv().await {
        match msg {
            Message::Join { client } => {
                let _ = clients.insert(client.client_id.clone(), client);
            }
            Message::Leave { client_id } => {
                let _ = clients.remove(&client_id);
            }
            Message::SetRole { client_id, role } => {
                let client = clients.get_mut(&client_id).ok_or_else(|| {
                    Error::General(format!("failed to find client: {:?}", client_id))
                })?;
                match client.role {
                    ClientRole::Undefined => {
                        client.role = role;
                    }
                    _ => {
                        let _ = client.send(Message::Error {
                            msg: "role validation failed".into(),
                        });
                    }
                }

                let _ = ticket_manager.flush_pending_tickets(&clients).await;
            }
            Message::PlateEvent {
                client_id,
                plate,
                timestamp,
            } => {
                let client = clients.get(&client_id).ok_or_else(|| {
                    Error::General(format!("failed to find client: {:?}", client_id))
                })?;
                match &client.role {
                    ClientRole::Camera { road, mile, limit } => {
                        if let Some(ticket) =
                            ticket_manager.add_plate_event(road, mile, limit, &plate, &timestamp)
                        {
                            ticket_manager.add_ticket(ticket);
                            let _ = ticket_manager.flush_pending_tickets(&clients).await;
                        }
                    }
                    other => {
                        return Err(Error::General(
                            "only camera should receive plate event".into(),
                        ));
                    }
                }
            }
            other => {
                return Err(Error::General(format!(
                    "unexpected message received: {:?}",
                    other
                )));
            }
        }
    }

    Ok(())
}

fn day_from_timestamp(ts: u32) -> u32 {
    ts / 86400
}

// Integer-only speed calculation: returns speed * 100
fn compute_speed_100x(delta_mile: u16, delta_time: u32) -> u16 {
    let dm = delta_mile as u64;
    let dt = delta_time as u64;
    ((dm * 360_000) / dt) as u16 // 3600 sec/hour * 100
}
