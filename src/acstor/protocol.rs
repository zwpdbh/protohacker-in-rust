use super::{PlannerMessage, WorkloadMessage};

pub enum Event {
    Planner(PlannerMessage),
    Workload(WorkloadMessage),
}
