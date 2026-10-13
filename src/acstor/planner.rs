use super::Event;
use crate::Result;
use tokio::sync::mpsc;

pub enum PlannerMessage {
    Hello,
    DoA,
    DoB,
    Stop,
}

pub struct Planner {
    planner_tx: mpsc::UnboundedSender<Event>,
    event_count: usize,
}

impl Planner {
    pub fn new(planner_tx: mpsc::UnboundedSender<Event>) -> Self {
        Planner {
            planner_tx,
            event_count: 0,
        }
    }

    pub async fn next_event(&mut self) -> PlannerMessage {
        self.event_count += 1;

        match self.event_count % 3 {
            0 => PlannerMessage::Hello,
            1 => PlannerMessage::DoA,
            2 => PlannerMessage::DoB,
            _ => PlannerMessage::Stop,
        }
    }

    async fn handle_event(&self, _event: Event) -> Result<()> {
        // Update internal state based on received events
        // Send myself an event
        // This could affect the next_event() behavior
        let _ = self.planner_tx.send(Event::Planner(PlannerMessage::Hello));
        Ok(())
    }
}

/// Based on planner's state and other events keep generating PlannerMessage
pub async fn generate_events(
    mut planner: Planner,
    tx: mpsc::UnboundedSender<Event>,
    mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    mut planner_rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
    loop {
        tokio::select! {
            Some(event) = planner_rx.recv() => {
                let _ = planner.handle_event(event).await;
            }
            planner_event = planner.next_event() => {
                if tx.send(Event::Planner(planner_event)).is_err() {
                    break;
                }
                // Explicitly yields control back to the tokio runtime scheduler, allowing other tasks to run before the current task continues.
                // When yield_now() is called:
                // The current task is placed at the end of the scheduler's queue
                // The scheduler immediately runs another available task
                // Your task continues only after other tasks have had a chance to execute
                tokio::task::yield_now().await;
            }
            _ = cancel_rx.recv() => {
                break;
            }
        }
    }
    Ok(())
}
