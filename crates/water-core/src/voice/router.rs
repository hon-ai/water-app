//! Deterministic voice router. Matches master spec § 6.2.
//!
//! Choice of speaker is determined entirely by:
//! - the trigger candidate (`preferred_track`, `trigger_id`)
//! - the scene snapshot (`characters_present`, `pov_character_id`)
//! - the persona registry (available speakers)
//! - the cooldown state (most-recent-emit timestamp per speaker)
//!
//! Variation lives only in the LLM's sampling; routing is pure + replayable.

use super::registry::PersonaRegistry;
use super::speaker::SpeakerArc;
use crate::orchestrator::TriggerCandidate;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Default)]
pub struct CooldownState {
    pub last_emit: HashMap<String, Instant>,
}

impl CooldownState {
    pub fn note_emit(&mut self, speaker_id: &str) {
        self.last_emit
            .insert(speaker_id.to_string(), Instant::now());
    }
}

/// Map a `trigger_id` → preferred persona id. This is the "default speaker track"
/// column in master spec § 6.1. Returned id may be overridden by routing rules.
fn default_persona_for_trigger(trigger_id: &str) -> &'static str {
    match trigger_id {
        "block_anchored_drift" => "editor",
        "topic_drift" | "pace_floor" => "architect",
        "structural_inflection" | "world_drift" => "cartographer",
        "no_universe_yet" => "chorus",
        // "scene_flow_dip" | "valence_spike" | _ → echo
        _ => "echo",
    }
}

/// Returns the chosen Speaker for this candidate. None if nothing is
/// available (every relevant speaker is cooled-down — caller skips this tick).
#[must_use]
pub fn route(
    candidate: &TriggerCandidate,
    personas: &PersonaRegistry,
    cooldowns: &CooldownState,
    now: Instant,
) -> Option<SpeakerArc> {
    // M2 ships persona-only routing. SpeakerTrack::Character is always treated
    // as "fall back to persona" until M3 wires the CharacterRegistry.
    let _ = candidate.preferred_track;

    let preferred_id = default_persona_for_trigger(candidate.trigger_id);

    // Filter out cooled-down speakers.
    let available: Vec<SpeakerArc> = personas
        .list()
        .iter()
        .filter(|s| {
            cooldowns.last_emit.get(s.id()).is_none_or(|last| {
                now.duration_since(*last).as_millis() >= u128::from(s.cooldown_ms())
            })
        })
        .cloned()
        .collect();
    if available.is_empty() {
        return None;
    }

    // Prefer the trigger's default persona if available; else LRU among non-cooled-down.
    if let Some(pref) = available.iter().find(|s| s.id() == preferred_id) {
        return Some(pref.clone());
    }
    // Tie-break: least-recently-used (or never-used).
    available
        .into_iter()
        .min_by_key(|s| cooldowns.last_emit.get(s.id()).copied().unwrap_or(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{SpeakerTrack, TriggerCandidate};
    use crate::Db;
    use tempfile::TempDir;

    fn registry() -> PersonaRegistry {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        PersonaRegistry::from_db(&db).unwrap()
    }

    fn cand(id: &'static str) -> TriggerCandidate {
        TriggerCandidate {
            trigger_id: id,
            priority: 5.0,
            preferred_track: SpeakerTrack::Either,
            reason: String::new(),
            block_target_id: None,
        }
    }

    #[test]
    fn block_anchored_drift_picks_editor() {
        let reg = registry();
        let s = route(
            &cand("block_anchored_drift"),
            &reg,
            &CooldownState::default(),
            Instant::now(),
        )
        .unwrap();
        assert_eq!(s.id(), "editor");
    }

    #[test]
    fn cooldown_skips_preferred_in_favor_of_lru() {
        let reg = registry();
        let mut cd = CooldownState::default();
        cd.note_emit("editor");
        let s = route(&cand("block_anchored_drift"), &reg, &cd, Instant::now()).unwrap();
        assert_ne!(s.id(), "editor");
    }

    #[test]
    fn no_universe_yet_picks_chorus() {
        let reg = registry();
        let s = route(
            &cand("no_universe_yet"),
            &reg,
            &CooldownState::default(),
            Instant::now(),
        )
        .unwrap();
        assert_eq!(s.id(), "chorus");
    }

    #[test]
    fn route_is_deterministic_for_same_inputs() {
        let reg = registry();
        let cd = CooldownState::default();
        let t = Instant::now();
        let s1 = route(&cand("topic_drift"), &reg, &cd, t).unwrap();
        let s2 = route(&cand("topic_drift"), &reg, &cd, t).unwrap();
        assert_eq!(s1.id(), s2.id());
    }
}
