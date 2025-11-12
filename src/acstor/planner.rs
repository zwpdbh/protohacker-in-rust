use super::Event;
use crate::Result;
use tokio::sync::mpsc;

pub enum PlannerMessage {
    Hello,

    DoA,
}

pub struct Planner {
    planner_tx: mpsc::UnboundedSender<Event>,
}

impl Planner {
    pub fn new(planner_tx: mpsc::UnboundedSender<Event>) -> Self {
        Planner { planner_tx }
    }

    pub async fn next_event(&self) -> PlannerMessage {
        PlannerMessage::Hello
    }

    async fn handle_event_from_workload(&self, _event: Event) -> Result<()> {
        // send myself a different message after some operation?
        // or alter the state to generate a differet next_event?
        let _x = self.planner_tx.send(Event::Planner(PlannerMessage::DoA));
        Ok(())
    }
}

/// Based on planner's state and other events keep generating PlannerMessage
pub async fn generate_events(
    planner: Planner,
    tx: mpsc::UnboundedSender<Event>,
    mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    mut planner_rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
    loop {
        tokio::select! {
            Some(event) = planner_rx.recv() => {
                let _ = planner.handle_event_from_workload(event).await;
            }
            planner_event = planner.next_event() => {
                if tx.send(Event::Planner(planner_event)).is_err() {
                    break;
                }
            }
            _ = cancel_rx.recv() => {
                break;
            }
        }
    }
    Ok(())
}
