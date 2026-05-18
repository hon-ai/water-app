//! Deterministic state machine for individual pill lifecycles.
//! Pure: `(state, event) -> (state, optional side-effect)`.

use crate::Id;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PillLifecycle {
    Generating, // LLM call in flight
    Surfacing,  // emit pill:emerged fired; renderer is fading-in
    OnScreen,   // visible to writer; eligible for click
    Pinned,     // moved to pinned column
    Dismissed,  // user X
    Expired,    // soft TTL elapsed
    Evicted,    // FIFO replaced by a newer candidate
}

#[derive(Debug, Clone)]
pub struct Pill {
    pub id: Id,
    pub state: PillLifecycle,
    pub created_at: Instant,
    pub speaker_id: String,
    pub trigger_id: String,
    pub text: Option<String>,
    pub block_target_id: Option<String>,
    pub parent_pill_id: Option<Id>,
}

impl Pill {
    #[must_use]
    pub fn new_generating(
        speaker_id: String,
        trigger_id: String,
        block_target_id: Option<String>,
        parent_pill_id: Option<Id>,
    ) -> Self {
        Self {
            id: Id::new(),
            state: PillLifecycle::Generating,
            created_at: Instant::now(),
            speaker_id,
            trigger_id,
            text: None,
            block_target_id,
            parent_pill_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PillEvent {
    LlmReturned { text: String },
    LlmFailed,
    UserPin,
    UserDismiss,
    Expired,
    Evicted,
    PostFilterDrop,
}

/// Apply an event to a pill. Returns the new state (which may be unchanged).
///
/// Arms are kept separate (not merged via `|`) so the table reads as a
/// state-transition diagram rather than a set of result-grouped patterns.
#[must_use]
#[allow(clippy::match_same_arms)]
pub fn transition(pill: &Pill, event: &PillEvent) -> PillLifecycle {
    use PillEvent::{
        Evicted, Expired, LlmFailed, LlmReturned, PostFilterDrop, UserDismiss, UserPin,
    };
    use PillLifecycle::{Dismissed, Generating, OnScreen, Pinned, Surfacing};
    match (pill.state, event) {
        (Generating, LlmReturned { .. }) => Surfacing,
        (Generating, LlmFailed | PostFilterDrop) => Dismissed,
        (Surfacing, _) => OnScreen,
        (OnScreen, UserPin) => Pinned,
        (OnScreen, UserDismiss) => Dismissed,
        (OnScreen, Expired) => PillLifecycle::Expired,
        (OnScreen, Evicted) => PillLifecycle::Evicted,
        (state, _) => state,
    }
}

/// Soft TTL after which an on-screen pill expires.
pub const PILL_SOFT_TTL: Duration = Duration::from_secs(90);

#[must_use]
pub fn should_expire(pill: &Pill, now: Instant) -> bool {
    pill.state == PillLifecycle::OnScreen
        && now.saturating_duration_since(pill.created_at) > PILL_SOFT_TTL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pill() -> Pill {
        Pill::new_generating("echo".into(), "block_anchored_drift".into(), None, None)
    }

    #[test]
    fn generating_to_surfacing_on_llm_return() {
        let p = pill();
        assert_eq!(
            transition(&p, &PillEvent::LlmReturned { text: "x".into() }),
            PillLifecycle::Surfacing
        );
    }
    #[test]
    fn generating_to_dismissed_on_failure() {
        let p = pill();
        assert_eq!(
            transition(&p, &PillEvent::LlmFailed),
            PillLifecycle::Dismissed
        );
    }
    #[test]
    fn onscreen_to_pinned_on_user_pin() {
        let mut p = pill();
        p.state = PillLifecycle::OnScreen;
        assert_eq!(transition(&p, &PillEvent::UserPin), PillLifecycle::Pinned);
    }
    #[test]
    fn terminal_states_absorb_events() {
        let mut p = pill();
        p.state = PillLifecycle::Dismissed;
        assert_eq!(
            transition(&p, &PillEvent::UserPin),
            PillLifecycle::Dismissed
        );
    }
}
