use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct IdlePauseWithPresentCharacter;

impl Trigger for IdlePauseWithPresentCharacter {
    fn id(&self) -> &'static str {
        "idle_pause_with_present_character"
    }

    /// M2 ships the slot; M3 wires character voices.
    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        None
    }
}
