use super::Event;
use super::planner::{self, Planner, PlannerMessage};
use crate::Result;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Duration;
use tracing::info;

pub struct WorkloadConfig {
    pub ticker_interval_ms: u64,
    pub max_events: Option<usize>,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            ticker_interval_ms: 3000,
            max_events: None,
        }
    }
}

pub enum WorkloadMessage {
    Tick,
}

pub struct Workload {
    workload_tx: mpsc::UnboundedSender<Event>,
    workload_rx: mpsc::UnboundedReceiver<Event>,
    planner_tx: mpsc::UnboundedSender<Event>,
    cancel_tx: broadcast::Sender<()>,
    config: WorkloadConfig,
    event_counter: usize,
}

impl Workload {
    pub fn new() -> (
        Self,
        mpsc::UnboundedSender<Event>,
        mpsc::UnboundedReceiver<Event>,
    ) {
        let (cancel_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let (workload_tx, workload_rx) = mpsc::unbounded_channel::<Event>();
        let (planner_tx, planner_rx) = mpsc::unbounded_channel::<Event>();

        (
            Workload {
                workload_tx,
                workload_rx,
                planner_tx: planner_tx.clone(),
                cancel_tx,
                config: WorkloadConfig::default(),
                event_counter: 0,
            },
            planner_tx,
            planner_rx,
        )
    }

    pub async fn run(
        &mut self,
        planner_tx: mpsc::UnboundedSender<Event>,
        planner_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Result<()> {
        // let (workload_tx, mut workload_rx) = mpsc::unbounded_channel::<Event>();
        let workload_tx_clone = self.workload_tx.clone();

        // let (planner_tx, mut planner_rx) = mpsc::unbounded_channel::<Event>();
        let planner = Planner::new(planner_tx);

        let mut planner_task = tokio::spawn(planner::generate_events(
            planner,
            self.workload_tx.clone(),
            self.cancel_tx.subscribe(),
            planner_rx,
        ));
        let mut workload_ticker_task = tokio::spawn(generate_events_from_time_ticker_with_cancel(
            workload_tx_clone,
            self.cancel_tx.subscribe(),
            self.config.ticker_interval_ms,
        ));

        loop {
            tokio::select! {
                Some(event) = self.workload_rx.recv() => {
                    match event {
                        Event::Planner(e) => {
                            let _ = self.handle_planner_event(e).await;
                        }
                        Event::Workload(e) => {
                            let _ = self.handle_workload_event(e).await;
                        }
                    }
                }
                _ = &mut planner_task => {
                    let _ = self.cancel_tx.send(());
                    break;
                }
                _ = &mut workload_ticker_task => {
                    let _ = self.cancel_tx.send(());
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_planner_event(&mut self, event: PlannerMessage) -> Result<()> {
        match event {
            PlannerMessage::Hello => {
                if self
                    .config
                    .max_events
                    .map_or(true, |max| self.event_counter < max)
                {
                    let _ = self
                        .workload_tx
                        .send(Event::Workload(WorkloadMessage::Tick));
                    self.event_counter += 1;
                }
            }
            PlannerMessage::DoA => {
                let _ = self.planner_tx.send(Event::Workload(WorkloadMessage::Tick));
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_workload_event(&self, event: WorkloadMessage) -> Result<()> {
        match event {
            WorkloadMessage::Tick => {
                info!("do something when tick")
            }
        }

        Ok(())
    }
}

pub async fn generate_events_from_time_ticker_with_cancel(
    tx: mpsc::UnboundedSender<Event>,
    mut cancel_rx: tokio::sync::broadcast::Receiver<()>,
    ticker_interval_ms: u64,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(ticker_interval_ms));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if tx.send(Event::Workload(WorkloadMessage::Tick)).is_err() {
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
