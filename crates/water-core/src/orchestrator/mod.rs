//! Pill orchestrator: deterministic state machine + trigger evaluation.
//! See docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md § 6.

pub mod anti_loop;
pub mod eviction;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerTrack {
    Persona,
    Character,
    Either,
}

#[derive(Debug, Clone)]
pub struct TriggerCandidate {
    pub trigger_id: &'static str,
    pub priority: f32,
    pub preferred_track: SpeakerTrack,
    pub reason: String,
    pub block_target_id: Option<String>,
}

pub trait Trigger: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate>;
}
