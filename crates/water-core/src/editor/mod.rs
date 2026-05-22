//! Phase 5 — Editor pills (UX_SPEC §E).
//!
//! Diagnostic engine + persistence for the *editor* class of pill —
//! distinct from the generative pills the orchestrator drives. Editor
//! pills are sticky: they emerge on save / debounced edit, persist
//! across sessions, and only disappear when the writer dismisses them
//! or the anchor span itself is gone.
//!
//! The engine is split:
//!   - `diagnostics`: pure functions that scan a block of prose and
//!     return findings. Stateless; testable in isolation.
//!   - `phrasebank`: per-rule Editor-voice template strings, picked
//!     round-robin so the same finding doesn't carry the same prose
//!     every time. Each template is screened against `tone.toml`'s
//!     blacklist before surfacing (UX_SPEC §E.7).
//!   - `store`: SQLite read/write against the `editor_pill` table
//!     (v10 migration). Owns the lifecycle — upsert, list, dismiss,
//!     cleanup.

pub mod diagnostics;
pub mod phrasebank;
pub mod store;

pub use diagnostics::{
    run_diagnostics_on_block, DiagnosticFinding, EditorRule, EditorSeverity,
};
pub use phrasebank::render_message;
pub use store::{compute_block_hash, EditorPillRow, EditorPillStore, ScanBlock};
