use crate::orchestrator::{Trigger, TriggerCandidate, TriggerContext};

pub struct CharacterDissonance;

impl Trigger for CharacterDissonance {
    fn id(&self) -> &'static str {
        "character_dissonance"
    }

    /// M2 ships the slot; M3 fills it against LSM v2.1 sheets.
    /// See `KNOWN_FRAGILE.md` #1 for the design rationale.
    fn evaluate(&self, _ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        None
    }
}
