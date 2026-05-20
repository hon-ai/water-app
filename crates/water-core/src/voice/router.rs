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
use crate::character::registry::CharacterRegistry;
use crate::orchestrator::{SceneSnapshot, SpeakerTrack, TriggerCandidate};
use std::collections::HashMap;
use std::time::Instant;

/// Trigger IDs that should prefer a character speaker (POV → present LRU)
/// before falling back to the persona track. Mirrors master spec § 6.2 +
/// the M3 character-track trigger fleet.
pub const CHAR_TRACK_TRIGGERS: &[&str] = &[
    "block_anchored_drift",
    "topic_drift",
    "valence_spike",
    "idle_pause_with_present_character",
    "character_dissonance",
];

/// Trigger IDs that always route to the **Cartographer** persona,
/// regardless of POV / character presence. M4 Task 14: only
/// `world_drift` for now; future world-track triggers (e.g. season
/// drift, location dissonance) will join this list.
///
/// These triggers bypass the character-track POV-prefer logic in
/// [`route_with_chars`] because their content is about the world,
/// not any individual character. Must NOT overlap with
/// [`CHAR_TRACK_TRIGGERS`] — see `world_track_triggers_does_not_collide_with_char_track`.
pub const WORLD_TRACK_TRIGGERS: &[&str] = &["world_drift"];

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

    // M4 Task 14: world-track triggers always route to Cartographer when
    // it's registered and not on cooldown. If Cartographer is cooled
    // down, fall through to the standard persona-rotation logic below
    // (which will pick the next-best non-cooled-down persona via LRU)
    // rather than skip the tick — preserves pre-Task-14 reachability.
    if WORLD_TRACK_TRIGGERS.contains(&candidate.trigger_id.as_str()) {
        if let Some(cart) = personas.list().iter().find(|s| s.id() == "cartographer") {
            let on_cooldown = cooldowns.last_emit.get(cart.id()).is_some_and(|last| {
                now.duration_since(*last).as_millis() < u128::from(cart.cooldown_ms())
            });
            if !on_cooldown {
                return Some(cart.clone());
            }
        }
    }

    let preferred_id = default_persona_for_trigger(&candidate.trigger_id);

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

/// Character-aware variant of [`route`].
///
/// Routing order for character-track triggers (see `CHAR_TRACK_TRIGGERS`)
/// when the candidate's `preferred_track` is `Character` or `Either`:
/// 1. The scene's POV character, if set, present, and not cooled down.
/// 2. LRU among the present, non-cooled-down characters
///    (`CharacterRegistry::pick_lru_present`).
/// 3. Persona fallback via [`route`].
///
/// All other cases (persona-track triggers, no characters present,
/// non-character-track trigger ids) fall straight through to [`route`].
#[must_use]
pub fn route_with_chars(
    candidate: &TriggerCandidate,
    personas: &PersonaRegistry,
    characters: &CharacterRegistry,
    scene: &SceneSnapshot,
    cooldowns: &CooldownState,
    now: Instant,
) -> Option<SpeakerArc> {
    let is_char_track = CHAR_TRACK_TRIGGERS.contains(&candidate.trigger_id.as_str())
        && (candidate.preferred_track == SpeakerTrack::Character
            || candidate.preferred_track == SpeakerTrack::Either);
    if is_char_track && !scene.characters_present.is_empty() {
        // 1) POV if set and present (and not on cooldown).
        if let Some(pov_id) = scene.pov_character_id.as_ref() {
            if scene.characters_present.contains(pov_id) {
                if let Some(speaker) = characters.by_id(pov_id.as_str()) {
                    let last = cooldowns.last_emit.get(speaker.id()).copied();
                    let on_cooldown = last.is_some_and(|t| {
                        now.duration_since(t).as_millis() < u128::from(speaker.cooldown_ms())
                    });
                    if !on_cooldown {
                        return Some(speaker);
                    }
                }
            }
        }
        // 2) LRU among present, non-cooled-down characters.
        if let Some(speaker) =
            characters.pick_lru_present(&scene.characters_present, &cooldowns.last_emit, now)
        {
            return Some(speaker);
        }
    }
    // 3) Fall through to persona routing (existing M2 logic).
    route(candidate, personas, cooldowns, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::registry::CharacterRegistry;
    use crate::orchestrator::{SceneSnapshot, SpeakerTrack, TriggerCandidate};
    use crate::voice::speaker::SpeakerKind;
    use crate::{Db, Id};
    use tempfile::TempDir;

    fn registry() -> PersonaRegistry {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        PersonaRegistry::from_db(&db).unwrap()
    }

    fn cand(id: &'static str) -> TriggerCandidate {
        TriggerCandidate {
            trigger_id: id.to_string(),
            priority: 5.0,
            ..Default::default()
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

    // ------------------------------------------------------------------
    // M3 T5: route_with_chars — POV-prefer + LRU fallback + persona fallback
    // ------------------------------------------------------------------

    /// 26-char Crockford-base32 ULID for the POV character used in tests.
    /// (Plan said `01HE000000000000000000POV1` but `O` is not valid in
    /// Crockford base32; substituted with a valid ULID-shaped string.)
    fn pov_character_id_str() -> &'static str {
        "01HE000000000000000000P0V1"
    }

    /// 26-char Crockford-base32 ULID for the second character ("OTHER")
    /// used in tests. Same substitution rationale as `pov_character_id_str`.
    fn other_character_id_str() -> &'static str {
        "01HE000000000000000000TH3R"
    }

    fn cand_with_track(id: &'static str, track: SpeakerTrack) -> TriggerCandidate {
        TriggerCandidate {
            trigger_id: id.to_string(),
            priority: 5.0,
            preferred_track: track,
            ..Default::default()
        }
    }

    fn setup_character_registry_with_pov() -> (TempDir, CharacterRegistry) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                rusqlite::params![],
            )
            .unwrap();
        let data = serde_json::json!({
            "main": { "full_name": "POV" },
            "bonus_traits": { "voice": "v" }
        })
        .to_string();
        db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
             VALUES (?1, 'p1', 'POV', 'lsm-v2.1', ?2, '--water-hue-character-1', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![pov_character_id_str(), data],
        ).unwrap();
        let data2 = serde_json::json!({
            "main": { "full_name": "OTHER" },
            "bonus_traits": { "voice": "v" }
        })
        .to_string();
        db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
             VALUES (?1, 'p1', 'OTHER', 'lsm-v2.1', ?2, '--water-hue-character-2', 'y', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z')",
            rusqlite::params![other_character_id_str(), data2],
        ).unwrap();
        let reg = CharacterRegistry::from_db(&db).unwrap();
        (dir, reg)
    }

    fn scene_with_pov_and_present() -> SceneSnapshot {
        SceneSnapshot {
            id: Id::new(),
            pov_character_id: Some(pov_character_id_str().parse::<Id>().unwrap()),
            location_id: None,
            characters_present: vec![
                pov_character_id_str().parse::<Id>().unwrap(),
                other_character_id_str().parse::<Id>().unwrap(),
            ],
            word_count: 500,
            seconds_since_last_pill: 60,
        }
    }

    #[test]
    fn pov_character_picked_for_character_track_trigger() {
        let persona_reg = registry();
        let (_tmp, char_reg) = setup_character_registry_with_pov();
        let cd = CooldownState::default();
        let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
        let scene = scene_with_pov_and_present();
        let s =
            route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
        assert_eq!(s.id(), pov_character_id_str());
        assert_eq!(s.kind(), SpeakerKind::Character);
    }

    #[test]
    fn pov_cooled_down_falls_to_lru_present_character() {
        let persona_reg = registry();
        let (_tmp, char_reg) = setup_character_registry_with_pov();
        let mut cd = CooldownState::default();
        cd.note_emit(pov_character_id_str());
        let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
        let scene = scene_with_pov_and_present();
        let s =
            route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
        assert_eq!(s.kind(), SpeakerKind::Character);
        assert_ne!(s.id(), pov_character_id_str());
    }

    #[test]
    fn no_present_characters_falls_back_to_persona() {
        let persona_reg = registry();
        let char_reg = CharacterRegistry::empty();
        let cd = CooldownState::default();
        let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        let s =
            route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
        assert_eq!(s.kind(), SpeakerKind::Persona);
        assert_eq!(s.id(), "editor"); // M2 default persona for block_anchored_drift
    }

    // ------------------------------------------------------------------
    // M4 T14: WORLD_TRACK_TRIGGERS — world_drift -> cartographer
    // ------------------------------------------------------------------

    #[test]
    fn world_drift_routes_to_cartographer() {
        let reg = registry();
        let s = route(
            &cand("world_drift"),
            &reg,
            &CooldownState::default(),
            Instant::now(),
        )
        .unwrap();
        assert_eq!(s.id(), "cartographer");
    }

    #[test]
    fn world_track_triggers_does_not_collide_with_char_track() {
        for t in WORLD_TRACK_TRIGGERS {
            assert!(
                !CHAR_TRACK_TRIGGERS.contains(t),
                "{t} appears in both WORLD_TRACK_TRIGGERS and CHAR_TRACK_TRIGGERS"
            );
        }
    }

    #[test]
    fn persona_track_trigger_unchanged_by_character_data() {
        let persona_reg = registry();
        let (_tmp, char_reg) = setup_character_registry_with_pov();
        let cd = CooldownState::default();
        // no_universe_yet is persona-track only
        let cand = cand_with_track("no_universe_yet", SpeakerTrack::Persona);
        let scene = scene_with_pov_and_present();
        let s =
            route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
        assert_eq!(s.kind(), SpeakerKind::Persona);
        assert_eq!(s.id(), "chorus");
    }
}
