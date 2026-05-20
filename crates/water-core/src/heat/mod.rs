//! Heatmap audiovisualizer (M5).
//!
//! The heat module owns the per-paragraph metric cache that drives
//! Water's per-scene heat strip. Five metrics:
//!
//! - **Pacing**     — rolling word-count density over the scene's typing
//!                    history. Local-only; no LLM cost.
//! - **Valence**    — per-paragraph emotional valence (-1 cold .. +1 warm).
//!                    LLM-backed; cached by text_hash so steady-state
//!                    recompute is ~5 calls per autosave.
//! - **Coherence**  — per-paragraph semantic continuity vs. the
//!                    preceding N paragraphs. LLM-backed (cosine via the
//!                    Claude API in M5; local embeddings in M6+).
//! - **Presence**   — count of character mentions per paragraph
//!                    (M3 autosuggest scanner reused).
//! - **WorldRefs**  — count of world-entry name+alias matches per
//!                    paragraph (M4 WorldRegistry::find_by_token reused).
//!
//! Persistence: rows live in [`heat_metric`] (per-paragraph cache, keyed
//! by text_hash for cache-skip) and [`scene_typing_history`] (append-only
//! ring for pacing). Both cascade on scene delete (v5 migration).

pub mod compute;
pub mod llm;
pub mod paragraph;
mod store;
mod types;

pub use compute::{compute_entity_mentions, compute_pacing, Entity, TypingEvent};
pub use llm::{compute_coherence, compute_valence};
pub use paragraph::{hash_text, partition, Paragraph};
pub use store::HeatStore;
pub use types::{HeatMetricKind, HeatRow};
