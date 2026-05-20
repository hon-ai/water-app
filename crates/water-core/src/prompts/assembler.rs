//! Assembler: composes `tone + speaker + trigger + task + inputs` into the
//! system/user prompt pair the LLM router will dispatch.

use super::loader::PromptLibrary;
use crate::voice::speaker::Speaker;

/// A complete LLM request ready for dispatch: the `system` block carries
/// tone + speaker + trigger + task; the `user` block carries inputs.
/// `expect_json` indicates whether the caller should use a structured-JSON
/// path on the provider (Task 18).
#[derive(Debug, Clone)]
pub struct PromptRequest {
    pub system: String,
    pub user: String,
    pub expect_json: bool,
}

/// Tone clauses in canonical order. Missing keys are skipped silently — the
/// loader has already validated that the embedded `tone.toml` parses.
fn tone_block(lib: &PromptLibrary) -> String {
    let order = [
        "present_tense",
        "not_assistant",
        "blacklist",
        "observe",
        "shape",
        "pass",
    ];
    let mut s = String::new();
    for k in order {
        if let Some(c) = lib.tone.clauses.get(k) {
            s.push_str(c);
            s.push('\n');
        }
    }
    s
}

/// Build the shared `system` block for any Level-0/expand/regenerate
/// assembly: tone clauses, then speaker identity, then trigger framing, then
/// the task instruction.
fn system_block(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    trigger_framing: &str,
    task_instruction: &str,
) -> String {
    format!(
        "{}\n[speaker: {}]\n{}\n[trigger: {}]\n{}\n[task]\n{}\n",
        tone_block(lib),
        speaker.display_name(),
        speaker.prompt_fragment(),
        trigger_id,
        trigger_framing,
        task_instruction,
    )
}

/// Assemble a Level-0 (single-pill) request.
pub fn assemble_level_0(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    scene_excerpt: &str,
) -> Result<PromptRequest, String> {
    let trig = lib
        .trigger(trigger_id)
        .ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib
        .task("pill_level_0")
        .ok_or_else(|| "pill_level_0 task missing".to_string())?;
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task.instruction);
    let user = format!("[inputs]\nManuscript excerpt:\n{scene_excerpt}");
    Ok(PromptRequest {
        system,
        user,
        expect_json: false,
    })
}

/// Assemble a pill-expand (bouquet of three variants) request.
pub fn assemble_pill_expand(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    parent_pill_text: &str,
    scene_excerpt: &str,
) -> Result<PromptRequest, String> {
    let trig = lib
        .trigger(trigger_id)
        .ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib
        .task("pill_expand")
        .ok_or_else(|| "pill_expand task missing".to_string())?;
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task.instruction);
    let user = format!(
        "[parent observation]\n{parent_pill_text}\n\n[manuscript excerpt]\n{scene_excerpt}"
    );
    Ok(PromptRequest {
        system,
        user,
        expect_json: true,
    })
}

/// Assemble a pill-regenerate request, substituting the prior first-words
/// list into the task's `{prior_first_words}` placeholder so the LLM can
/// avoid repeating them.
pub fn assemble_pill_regenerate(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    parent_pill_text: &str,
    scene_excerpt: &str,
    prior_first_words: &[String],
) -> Result<PromptRequest, String> {
    let trig = lib
        .trigger(trigger_id)
        .ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib
        .task("pill_regenerate")
        .ok_or_else(|| "pill_regenerate task missing".to_string())?;
    let prior = prior_first_words
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let task_instruction = task.instruction.replace("{prior_first_words}", &prior);
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task_instruction);
    let user = format!(
        "[parent observation]\n{parent_pill_text}\n\n[manuscript excerpt]\n{scene_excerpt}"
    );
    Ok(PromptRequest {
        system,
        user,
        expect_json: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::registry::PersonaRegistry;
    use crate::voice::speaker::SpeakerArc;
    use crate::Db;
    use tempfile::TempDir;

    fn echo() -> (TempDir, SpeakerArc) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let s = reg.by_id("echo").unwrap();
        (dir, s)
    }

    #[test]
    fn level_0_includes_tone_clauses_and_speaker_and_trigger() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let (_tmp, e) = echo();
        let req = assemble_level_0(
            &lib,
            &*e,
            "block_anchored_drift",
            "She walked across the square.",
        )
        .unwrap();
        assert!(req.system.contains("Speak in present tense"));
        assert!(req.system.contains("Echo"));
        assert!(req.system.contains("listening through fog"));
        assert!(req.system.contains("block_anchored_drift"));
        assert!(req.user.contains("square"));
        assert!(!req.expect_json);
    }

    /// M4 Task 18: confirm Cartographer + `world_drift` assembles cleanly
    /// through the existing Level-0 path. This is the architectural contract
    /// the M4 plan implied with "real Cartographer voice template": the
    /// world-track trigger framing arrives via the trigger TOML, and the
    /// reactive-observational voice arrives via the persona TOML — there is
    /// no separate Mustache renderer, just the existing `assemble_level_0`.
    #[test]
    fn cartographer_world_drift_level_0_assembly_is_clean() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let cart = reg.by_id("cartographer").unwrap();
        let req = assemble_level_0(
            &lib,
            &*cart,
            "world_drift",
            "The Pell Library opened its doors at dusk, though no one had said the name aloud in a year.",
        )
        .unwrap();

        // Cartographer voice surfaces.
        assert!(req.system.contains("Cartographer"));
        assert!(req.system.contains("notice") || req.system.contains("notices"));
        // World-drift trigger framing surfaces.
        assert!(req.system.contains("world_drift"));
        assert!(req.system.contains("named entity"));
        // Scene excerpt is in the user block, not the system block.
        assert!(req.user.contains("Pell Library"));
        // The tone block carries the blacklist clause that explicitly forbids
        // the phrases. We assert it survives into the assembled system prompt
        // so the LLM sees the guardrail.
        assert!(req.system.contains("Never say"));
    }

    #[test]
    fn regenerate_substitutes_prior_first_words() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let (_tmp, e) = echo();
        let req = assemble_pill_regenerate(
            &lib,
            &*e,
            "topic_drift",
            "the rain hesitates",
            "more text",
            &["the rain hesitates".to_string(), "a small bell".to_string()],
        )
        .unwrap();
        assert!(req.system.contains("the rain hesitates"));
        assert!(req.system.contains("a small bell"));
        assert!(req.expect_json);
    }
}
