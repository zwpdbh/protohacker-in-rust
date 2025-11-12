use super::Event;
use crate::Result;
use tokio::sync::mpsc;

pub enum PlannerMessage {
    Hello,
    #[allow(unused)]
    DoA,
}

pub struct Planner {}

impl Planner {
    pub fn new() -> Self {
        Planner {}
    }

    pub async fn next_event(&self) -> PlannerMessage {
        PlannerMessage::Hello
    }

    async fn handle_event_from_workload(&self, _event: Event) -> Result<()> {
        Ok(())
    }
}

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
