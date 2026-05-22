//! Pill orchestrator: deterministic state machine + trigger evaluation.
//! See docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md § 6.

pub mod anti_loop;
pub mod arc;
pub mod eviction;
pub mod feedback;
pub mod lemma_overlap;
pub mod state;
pub mod triggers;

use crate::Id;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingTelemetry {
    pub idle_for_ms: u64,
    pub cursor_classification: CursorClassification,
    pub block_id: String,
    pub recent_word_delta: i32,
    pub structural_inflection: StructuralInflection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CursorClassification {
    AtSentenceEnd,
    AtParagraphEnd,
    MidSentence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StructuralInflection {
    NewScene,
    NewChapter,
    PovChange,
    LocationChange,
    None,
}

#[derive(Debug, Default, Clone)]
pub struct AnalysisSnapshot {
    pub flow: Option<f32>,
    pub coherence: Option<f32>,
    pub engagement: Option<f32>,
    pub divergence: Option<f32>,
    pub pace: Option<f32>,
    pub intensity: Option<f32>,
    pub valence: Option<f32>,
    pub block_metrics: std::collections::HashMap<String, BlockMetrics>,
    /// Most recent valence reading for the scene (used by `valence_spike`).
    pub valence_history: Vec<f32>,
    /// Text of the most recently-finished paragraph. Provided by the
    /// renderer's `typing:telemetry` events when `idle_for_ms >= 3000`.
    /// Used by `character_dissonance` to gate against character fields.
    pub last_block_text: Option<String>,
    /// M5: average pacing across the trailing 3 paragraphs from the
    /// `HeatStore` cache. Populated by the orchestrator service after
    /// each heat-compute pass. `pace_floor` prefers this over the
    /// per-tick `pace` heuristic when available — same gate semantics,
    /// but the signal is computed once over a longer window rather
    /// than re-derived per telemetry event.
    pub heat_pace_tail: Option<f32>,
    /// M5: average coherence across the trailing 3 paragraphs from the
    /// `HeatStore` cache. `scene_flow_dip` prefers this over the
    /// per-tick `coherence` heuristic when available.
    pub heat_coherence_tail: Option<f32>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BlockMetrics {
    pub flow: Option<f32>,
    pub coherence: Option<f32>,
    pub divergence: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct SceneSnapshot {
    pub id: Id,
    pub pov_character_id: Option<Id>,
    pub location_id: Option<Id>,
    pub characters_present: Vec<Id>,
    pub word_count: u32,
    pub seconds_since_last_pill: u64,
    /// Phase 6 — 0-indexed position of this scene in its manuscript.
    /// Used (with `manuscript_scene_count`) to derive an arc-position
    /// label for the prompt context. Optional during the renderer-side
    /// rollout; absent → assembler omits the "Position in arc" line.
    pub scene_ordering: Option<u32>,
    /// Phase 6 — total scenes in the manuscript. See `scene_ordering`.
    pub manuscript_scene_count: Option<u32>,
}

#[derive(Debug, Default, Clone)]
pub struct ProjectSnapshot {
    pub character_count: u32,
    pub world_entry_count: u32,
}

#[derive(Debug, Clone)]
pub struct TriggerContext<'a> {
    pub telemetry: &'a TypingTelemetry,
    pub analysis: &'a AnalysisSnapshot,
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
    pub characters: &'a crate::character::registry::CharacterRegistry,
    /// World Bible snapshot built once per dispatch in
    /// `orchestrator_service.rs` (M4 Task 13). Empty default is
    /// acceptable for tests that don't exercise world-track logic.
    pub world_registry: &'a crate::world::WorldRegistry,
    pub prompts: &'a crate::prompts::loader::PromptLibrary,
    /// v8: per-trigger learned sensitivity. Absent ids fall back to
    /// `feedback::SENSITIVITY_DEFAULT` (0.5), which preserves the
    /// pre-learning behavior. Triggers consume this via
    /// `feedback::loosen_above` / `loosen_below` on their numeric
    /// thresholds.
    pub tuning: &'a crate::orchestrator::feedback::TriggerTuning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerTrack {
    Persona,
    Character,
    Either,
}

/// A small system+user pair sent to the LLM as a yes/no gate before
/// proceeding with level-0 pill generation. Used today by
/// `character_dissonance` Stage 2; reusable for any future two-stage
/// trigger. Cheap by design: ~150 tokens in, 1 token out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// System prompt (instructive role copy).
    pub system: String,
    /// User prompt with all variables already substituted.
    pub user: String,
    /// Tag for telemetry / replay-log filtering. Currently only
    /// `"pill_dissonance_check"` but other two-stage triggers would
    /// add more variants.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCandidate {
    pub trigger_id: String,
    pub priority: f32,
    pub preferred_track: SpeakerTrack,
    pub reason: String,
    pub block_target_id: Option<String>,
    /// When `Some`, the orchestrator runs `ConfirmationRequest` as a
    /// yes/no LLM call before dispatching the level-0 prompt. When the
    /// confirmation returns "no" (or any non-"yes" string), the candidate
    /// is dropped without emitting a pill.
    ///
    /// New in M3. Defaults to `None` for backward-compat with M2 replay
    /// logs and any existing trigger that doesn't need two-stage gating.
    #[serde(default)]
    pub requires_confirmation: Option<ConfirmationRequest>,
}

impl Default for TriggerCandidate {
    fn default() -> Self {
        Self {
            trigger_id: String::new(),
            priority: 0.0,
            preferred_track: SpeakerTrack::Either,
            reason: String::new(),
            block_target_id: None,
            requires_confirmation: None,
        }
    }
}

pub trait Trigger: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate>;
}

/// Shared test fixtures for trigger evaluation. Exposes a single,
/// process-lifetime `PromptLibrary` so each test that builds a
/// `TriggerContext` does not re-parse all of the embedded TOML.
#[cfg(test)]
pub mod test_util {
    use crate::prompts::loader::PromptLibrary;
    use std::sync::OnceLock;

    /// Return a borrow of a `'static` `PromptLibrary` initialized exactly
    /// once for the test binary. Avoids per-test TOML parsing across the
    /// dozens of trigger tests now that `TriggerContext` carries a
    /// `prompts` field.
    ///
    /// # Panics
    /// Panics if the built-in TOML fails to parse — an unrecoverable
    /// test-environment bug equivalent to the production startup failure
    /// in `OrchestratorService::new`.
    pub fn test_prompts() -> &'static PromptLibrary {
        static LIB: OnceLock<PromptLibrary> = OnceLock::new();
        LIB.get_or_init(|| PromptLibrary::load_builtin().expect("built-in prompts must load"))
    }

    /// Return a borrow of a `'static` empty `WorldRegistry` for tests that
    /// don't need world-track data. Mirrors `test_prompts` so existing
    /// `TriggerContext { ... }` literals can pass `world_registry: test_world_registry()`
    /// without per-test setup.
    pub fn test_world_registry() -> &'static crate::world::WorldRegistry {
        static REG: OnceLock<crate::world::WorldRegistry> = OnceLock::new();
        REG.get_or_init(crate::world::WorldRegistry::default)
    }

    /// Return a borrow of a `'static` default (empty) `TriggerTuning`
    /// for tests that don't exercise learned-sensitivity behavior.
    /// Absent ids resolve to `SENSITIVITY_DEFAULT`, so tests that
    /// existed before v8 keep the same thresholds.
    pub fn test_tuning() -> &'static crate::orchestrator::feedback::TriggerTuning {
        static TUN: OnceLock<crate::orchestrator::feedback::TriggerTuning> = OnceLock::new();
        TUN.get_or_init(crate::orchestrator::feedback::TriggerTuning::default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_candidate_default_has_no_confirmation() {
        let c = TriggerCandidate::default();
        assert!(c.requires_confirmation.is_none());
    }

    #[test]
    fn trigger_candidate_with_confirmation_serializes_round_trip() {
        let original = TriggerCandidate {
            trigger_id: "character_dissonance".to_string(),
            priority: 5.5,
            preferred_track: SpeakerTrack::Character,
            reason: "test".to_string(),
            block_target_id: Some("block-1".to_string()),
            requires_confirmation: Some(ConfirmationRequest {
                system: "sys".to_string(),
                user: "usr".to_string(),
                kind: "pill_dissonance_check".to_string(),
            }),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: TriggerCandidate = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.requires_confirmation.as_ref().unwrap().kind,
            "pill_dissonance_check"
        );
    }

    #[test]
    fn trigger_context_carries_world_registry() {
        // M4 Task 13: TriggerContext gains a `world_registry` field.
        // Seed the 6 built-in segments and verify the context exposes them.
        let dir = tempfile::tempdir().unwrap();
        let db = crate::Db::open_in_memory().unwrap();
        let p = crate::ProjectStore::new(&db).insert("P").unwrap();
        crate::world::WorldStore::new(&db, dir.path().to_path_buf())
            .seed_builtins(&p.id)
            .unwrap();
        let world_reg =
            crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();

        let telem = TypingTelemetry {
            idle_for_ms: 0,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "b".into(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: crate::Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 0,
            seconds_since_last_pill: 0,
            scene_ordering: None,
            manuscript_scene_count: None,
        };
        let project = ProjectSnapshot::default();
        let characters = crate::character::registry::CharacterRegistry::empty();
        let prompts = test_util::test_prompts();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            world_registry: &world_reg,
            prompts,
            tuning: test_util::test_tuning(),
        };
        assert_eq!(ctx.world_registry.segments().count(), 6);
    }

    #[test]
    fn trigger_candidate_missing_confirmation_field_deserializes_as_none() {
        let m2_json = r#"{
            "trigger_id": "topic_drift",
            "priority": 5.0,
            "preferred_track": "persona",
            "reason": "test",
            "block_target_id": null
        }"#;
        let parsed: TriggerCandidate = serde_json::from_str(m2_json).unwrap();
        assert!(parsed.requires_confirmation.is_none());
    }
}
