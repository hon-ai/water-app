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

/// A two-stage confirmation prompt (e.g. `"pill_dissonance_check"`), with
/// `{{var}}` placeholders rendered at call time via
/// [`PromptLibrary::render_confirmation_request`]. Differs from `TaskPrompt`
/// in carrying separate `system` and `user` strings plus a `max_tokens`
/// budget — confirmations are cheap yes/no LLM calls, not generation tasks.
#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPrompt {
    pub version: String,
    pub id: String,
    pub prompt: ConfirmationPromptBody,
    pub output: ConfirmationPromptOutput,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPromptBody {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPromptOutput {
    pub format: String,
    pub max_tokens: u32,
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

const TASK_PILL_DISSONANCE_CHECK: &str =
    include_str!("../../../../prompts/tasks/pill_dissonance_check.toml");

/// Holds all built-in prompts in memory. Built once at startup via
/// [`PromptLibrary::load_builtin`] and shared (typically behind `Arc`) across
/// the orchestrator.
#[derive(Debug)]
pub struct PromptLibrary {
    pub tone: ToneClauses,
    pub triggers: HashMap<String, TriggerPrompt>,
    pub tasks: HashMap<String, TaskPrompt>,
    pub confirmations: HashMap<String, ConfirmationPrompt>,
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

        let mut confirmations: HashMap<String, ConfirmationPrompt> = HashMap::new();
        // One confirmation today (`pill_dissonance_check`); the loop shape
        // matches `triggers`/`tasks` above so adding a second is a one-line
        // change at the array literal.
        #[allow(clippy::single_element_loop)]
        for src in [TASK_PILL_DISSONANCE_CHECK] {
            let p: ConfirmationPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
            confirmations.insert(p.id.clone(), p);
        }

        Ok(Self {
            tone,
            triggers,
            tasks,
            confirmations,
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

    /// Look up a confirmation prompt by its `id` (e.g. `"pill_dissonance_check"`).
    #[must_use]
    pub fn confirmation(&self, id: &str) -> Option<&ConfirmationPrompt> {
        self.confirmations.get(id)
    }

    /// Render a confirmation prompt's system+user strings with `{{var}}`
    /// substitutions and wrap them in a [`ConfirmationRequest`] tagged with
    /// `kind = id`. Missing variables remain as literal `{{var}}` in the
    /// output so failures are LLM-visible — better a bad prompt than a
    /// silently-dropped variable.
    ///
    /// Returns `Err` only when the `id` is unknown.
    pub fn render_confirmation_request(
        &self,
        id: &str,
        vars: &[(&str, &str)],
    ) -> Result<crate::orchestrator::ConfirmationRequest, String> {
        let prompt = self
            .confirmation(id)
            .ok_or_else(|| format!("confirmation prompt not found: {id}"))?;
        let mut sys = prompt.prompt.system.clone();
        let mut usr = prompt.prompt.user.clone();
        for (k, v) in vars {
            let needle = format!("{{{{{k}}}}}");
            sys = sys.replace(&needle, v);
            usr = usr.replace(&needle, v);
        }
        Ok(crate::orchestrator::ConfirmationRequest {
            system: sys,
            user: usr,
            kind: id.to_string(),
        })
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
        assert_eq!(lib.confirmations.len(), 1);
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

    #[test]
    fn library_loads_pill_dissonance_check_confirmation() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let c = lib
            .confirmation("pill_dissonance_check")
            .expect("pill_dissonance_check confirmation must load");
        assert_eq!(c.id, "pill_dissonance_check");
        assert_eq!(c.version, "1");
        assert_eq!(c.output.format, "plain");
        assert_eq!(c.output.max_tokens, 4);
        assert!(
            c.prompt.user.contains("{{full_name}}"),
            "user template must contain {{{{full_name}}}} placeholder"
        );
        assert!(c.prompt.user.contains("{{field_label}}"));
        assert!(c.prompt.user.contains("{{field_value}}"));
        assert!(c.prompt.user.contains("{{paragraph_text}}"));
    }

    #[test]
    fn render_confirmation_request_substitutes_all_vars() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let req = lib
            .render_confirmation_request(
                "pill_dissonance_check",
                &[
                    ("full_name", "Marcus"),
                    ("field_label", "values"),
                    ("field_value", "loyalty, showing up"),
                    ("paragraph_text", "He walked away without looking back."),
                ],
            )
            .expect("render must succeed for known id");
        assert_eq!(req.kind, "pill_dissonance_check");
        assert!(
            !req.user.contains("{{"),
            "no {{ placeholders should remain after substitution; got: {}",
            req.user
        );
        assert!(req.user.contains("Marcus"));
        assert!(req.user.contains("values"));
        assert!(req.user.contains("loyalty, showing up"));
        assert!(req.user.contains("He walked away without looking back."));
        assert!(req
            .system
            .contains("contradicts a character's stated belief"));
    }

    #[test]
    fn render_confirmation_request_unknown_id_errors() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let err = lib
            .render_confirmation_request("not_a_real_id", &[])
            .expect_err("unknown id must error");
        assert!(
            err.contains("not_a_real_id"),
            "error message should mention the missing id; got: {err}"
        );
    }
}
