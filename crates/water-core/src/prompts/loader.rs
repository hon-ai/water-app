//! `PromptLibrary`: loads tone, triggers, and tasks from compile-time
//! `include_str!` of the workspace `prompts/` tree.

use serde::Deserialize;
use std::collections::HashMap;

/// Global tone clauses (named keys → text) plus the blacklist regex set
/// consumed by `ToneBlacklistFilter` in Task 18.
#[derive(Debug, Deserialize, Clone)]
pub struct ToneClauses {
    pub version: String,
    pub clauses: HashMap<String, String>,
    pub blacklist_regex: BlacklistPatterns,
}

/// Wrapper around the `[blacklist_regex].patterns` array in `tone.toml`.
#[derive(Debug, Deserialize, Clone)]
pub struct BlacklistPatterns {
    pub patterns: Vec<String>,
}

/// A trigger's framing prose, keyed by `id` (e.g. `"topic_drift"`).
#[derive(Debug, Deserialize, Clone)]
pub struct TriggerPrompt {
    pub version: String,
    pub id: String,
    pub framing: String,
}

/// A task's instruction prose, keyed by `id` (e.g. `"pill_level_0"`).
/// `output_format` is `"plain"` or `"json"` and drives `PromptRequest::expect_json`.
#[derive(Debug, Deserialize, Clone)]
pub struct TaskPrompt {
    pub version: String,
    pub id: String,
    pub instruction: String,
    pub output_format: String,
}

const TONE: &str = include_str!("../../../../prompts/tone.toml");

const TRIGGER_BLOCK_ANCHORED_DRIFT: &str =
    include_str!("../../../../prompts/triggers/block_anchored_drift.toml");
const TRIGGER_SCENE_FLOW_DIP: &str =
    include_str!("../../../../prompts/triggers/scene_flow_dip.toml");
const TRIGGER_TOPIC_DRIFT: &str = include_str!("../../../../prompts/triggers/topic_drift.toml");
const TRIGGER_VALENCE_SPIKE: &str = include_str!("../../../../prompts/triggers/valence_spike.toml");
const TRIGGER_STRUCTURAL_INFLECTION: &str =
    include_str!("../../../../prompts/triggers/structural_inflection.toml");
const TRIGGER_PACE_FLOOR: &str = include_str!("../../../../prompts/triggers/pace_floor.toml");
const TRIGGER_WORLD_DRIFT: &str = include_str!("../../../../prompts/triggers/world_drift.toml");
const TRIGGER_NO_UNIVERSE_YET: &str =
    include_str!("../../../../prompts/triggers/no_universe_yet.toml");
const TRIGGER_CHARACTER_DISSONANCE: &str =
    include_str!("../../../../prompts/triggers/character_dissonance.toml");
const TRIGGER_IDLE_PAUSE: &str =
    include_str!("../../../../prompts/triggers/idle_pause_with_present_character.toml");

const TASK_PILL_LEVEL_0: &str = include_str!("../../../../prompts/tasks/pill_level_0.toml");
const TASK_PILL_EXPAND: &str = include_str!("../../../../prompts/tasks/pill_expand.toml");
const TASK_PILL_REGENERATE: &str = include_str!("../../../../prompts/tasks/pill_regenerate.toml");

/// Holds all built-in prompts in memory. Built once at startup via
/// [`PromptLibrary::load_builtin`] and shared (typically behind `Arc`) across
/// the orchestrator.
pub struct PromptLibrary {
    pub tone: ToneClauses,
    pub triggers: HashMap<String, TriggerPrompt>,
    pub tasks: HashMap<String, TaskPrompt>,
}

impl PromptLibrary {
    /// Load the built-in tone + 10 triggers + 3 tasks from compile-time
    /// embedded TOML. Returns an error if any embedded TOML fails to parse —
    /// this is effectively a fatal startup condition.
    pub fn load_builtin() -> Result<Self, String> {
        let tone: ToneClauses = toml::from_str(TONE).map_err(|e| e.to_string())?;

        let mut triggers: HashMap<String, TriggerPrompt> = HashMap::new();
        for src in [
            TRIGGER_BLOCK_ANCHORED_DRIFT,
            TRIGGER_SCENE_FLOW_DIP,
            TRIGGER_TOPIC_DRIFT,
            TRIGGER_VALENCE_SPIKE,
            TRIGGER_STRUCTURAL_INFLECTION,
            TRIGGER_PACE_FLOOR,
            TRIGGER_WORLD_DRIFT,
            TRIGGER_NO_UNIVERSE_YET,
            TRIGGER_CHARACTER_DISSONANCE,
            TRIGGER_IDLE_PAUSE,
        ] {
            let p: TriggerPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
            triggers.insert(p.id.clone(), p);
        }

        let mut tasks: HashMap<String, TaskPrompt> = HashMap::new();
        for src in [TASK_PILL_LEVEL_0, TASK_PILL_EXPAND, TASK_PILL_REGENERATE] {
            let p: TaskPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
            tasks.insert(p.id.clone(), p);
        }

        Ok(Self {
            tone,
            triggers,
            tasks,
        })
    }

    /// Look up a trigger by its `id` (e.g. `"topic_drift"`).
    #[must_use]
    pub fn trigger(&self, id: &str) -> Option<&TriggerPrompt> {
        self.triggers.get(id)
    }

    /// Look up a task by its `id` (e.g. `"pill_level_0"`).
    #[must_use]
    pub fn task(&self, id: &str) -> Option<&TaskPrompt> {
        self.tasks.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_loads_all_built_in_prompts() {
        let lib = PromptLibrary::load_builtin().unwrap();
        assert_eq!(lib.tone.version, "1");
        assert_eq!(lib.triggers.len(), 10);
        assert_eq!(lib.tasks.len(), 3);
        assert!(lib
            .tone
            .blacklist_regex
            .patterns
            .iter()
            .any(|p| p.contains("you should")));
    }

    #[test]
    fn trigger_lookup_by_id() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let t = lib.trigger("topic_drift").unwrap();
        assert!(t.framing.contains("coherence"));
    }
}
