use super::client::*;
use super::protocol::*;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug, Clone)]
pub struct StateTx {
    sender: mpsc::UnboundedSender<Message>,
}

pub struct StateChannel {
    #[allow(unused)]
    sender: mpsc::UnboundedSender<Message>,
    receiver: mpsc::UnboundedReceiver<Message>,
}

impl StateChannel {
    async fn recv(&mut self) -> Option<Message> {
        self.receiver.recv().await
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
            sender: client_tx,
            receiver: client_rx,
        });
    }

    pub fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|e| Error::General(e.to_string()))
    }

    pub fn leave(&self, client_id: ClientId) -> Result<()> {
        self.sender
            .send(Message::Leave { client_id })
            .map_err(|_| Error::General("State channel closed".into()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Plate(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RoadInfo {
    road: u16,
    limit: u16,
}

struct Mile(u16);

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Timestamp(u32);

struct PlateTracker {
    plate_events: HashMap<Plate, BTreeMap<Timestamp, Mile>>,
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
}

#[derive(Debug)]
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
    fn new() -> Self {
        TicketManager {
            roads: HashMap::new(),
            ticketed: HashSet::new(),
            pending_tickets: vec![],
        }
    }

    fn add_ticket(&mut self, ticket: Ticket) {
        self.pending_tickets.push(ticket);
    }

    fn flush_pending_tickets(&mut self, clients: &HashMap<ClientId, Client>) -> Result<()> {
        let tickets = std::mem::take(&mut self.pending_tickets);
        if tickets.is_empty() {
            return Ok(());
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
                let _ = client
                    .sender
                    .send(Message::Ticket {
                        plate: ticket.plate.into(),
                        road: ticket.road,
                        mile1: ticket.mile1,
                        timestamp1: ticket.timestamp1,
                        mile2: ticket.mile2,
                        timestamp2: ticket.timestamp2,
                        speed: ticket.speed,
                    })
                    .map_err(|e| Error::General(e.to_string()))?;
            } else {
                info!(
                    "no associated dispatcher, so store the ticket: {:?}",
                    ticket
                );
                tickets_to_keep.push(ticket);
            }
        }

        self.pending_tickets = tickets_to_keep;
        Ok(())
    }

    // add a new plate event and generate a Option<Ticket>
    fn add_plate_observation(
        &mut self,
        road: u16,
        mile: u16,
        limit: u16,
        plate: &str,
        timestamp: u32,
    ) -> Option<Ticket> {
        let road_info = RoadInfo { road, limit };
        let plate_key = Plate(plate.to_string());
        let mile_val = Mile(mile);
        let ts_val = Timestamp(timestamp);

        // Get or create PlateTracker for this road
        let tracker = self
            .roads
            .entry(road_info.clone())
            .or_insert_with(PlateTracker::new);

        // Get or create PlateEvents for this plate
        let plate_events = tracker
            .plate_events
            .entry(plate_key.clone())
            .or_insert_with(|| BTreeMap::new());

        // Add new event
        plate_events.insert(ts_val, mile_val);

        // Convert to a vec for easy adjacent access (BTreeMap doesn't support direct indexing)
        let events: Vec<(&Timestamp, &Mile)> = plate_events.iter().collect();

        // Check all adjacent pairs
        for i in 0..events.len().saturating_sub(1) {
            let (ts1, m1) = events[i];
            let (ts2, m2) = events[i + 1];

            let delta_time = ts2.0 - ts1.0;
            if delta_time == 0 {
                continue; // shouldn't happen due to BTreeMap dedup, but safe
            }

            let delta_mile = m2.0.abs_diff(m1.0);
            let speed_100x = compute_speed_100x(delta_mile, delta_time);
            let threshold = road_info.limit * 100 + 50;

            if speed_100x < threshold {
                continue;
            }

            let day1 = day_from_timestamp(ts1.0);
            let day2 = day_from_timestamp(ts2.0);

            // Check if already ticketed on any day in [day1, day2]
            let violates_limit =
                (day1..=day2).any(|day| self.ticketed.contains(&(plate_key.clone(), day)));

            if violates_limit {
                continue;
            }

            // Mark all days in range as ticketed
            for day in day1..=day2 {
                self.ticketed.insert((plate_key.clone(), day));
            }

            return Some(Ticket {
                plate: plate.to_string(),
                road: road_info.road,
                mile1: m1.0,
                timestamp1: ts1.0,
                mile2: m2.0,
                timestamp2: ts2.0,
                speed: speed_100x,
            });
        }

        None
    }
}

async fn run_state(mut state_channel: StateChannel) -> Result<()> {
    // initalize state
    let mut clients: HashMap<ClientId, Client> = HashMap::new();
    let mut ticket_manager = TicketManager::new();
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
            Message::DispatcherOnline { client_id, roads } => {
                let client = clients.get_mut(&client_id).unwrap();
                client.role = ClientRole::Dispatcher { roads };
                info!("dispatcher is online: {client:?}, flush_pending_tickets");
                let _ = ticket_manager.flush_pending_tickets(&clients)?;
            }
            Message::PlateObservation {
                client_id,
                road,
                mile,
                limit,
                plate,
                timestamp,
            } => {
                info!(
                    "client: {client_id:?} observe plate: {plate}, road: {road}, limit: {limit}, timestamp: {timestamp}"
                );

                if let Some(ticket) =
                    ticket_manager.add_plate_observation(road, mile, limit, &plate, timestamp)
                {
                    info!("new ticket generated, ticket: {:?}", ticket);
                    ticket_manager.add_ticket(ticket);
                }
                let _ = ticket_manager.flush_pending_tickets(&clients)?;
            }
            other => {
                error!("unexpected msg: {:?}", other);
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
