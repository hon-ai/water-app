use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct WorldDrift;

impl Trigger for WorldDrift {
    fn id(&self) -> &'static str {
        "world_drift"
    }

    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        // M4 wires this against the World Bible. M2 ships the slot.
        None
    }
}
