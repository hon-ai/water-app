//! Prompt library: TOML loader + assembler. See spec § 8.
//!
//! Loads global tone clauses, 10 trigger framings, and 3 task instructions
//! at compile time via `include_str!`, then assembles complete
//! system/user prompt pairs by composing `tone + speaker + trigger + task +
//! inputs`. The assembled `PromptRequest` is the unit the LLM router consumes.

pub mod assembler;
pub mod loader;

pub use assembler::{
    assemble_editor_polish, assemble_level_0, assemble_pill_expand, assemble_pill_regenerate,
    assemble_rabbit_deepen_inherit, assemble_rabbit_fan_4, PromptContext, PromptRequest,
};
pub use loader::{PromptLibrary, TaskPrompt, ToneClauses, TriggerPrompt};
