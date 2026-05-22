//! Assembler: composes `tone + speaker + trigger + task + inputs` into the
//! system/user prompt pair the LLM router will dispatch.
//!
//! Phase 6 adds a `PromptContext` carrying optional richer-context fields
//! (arc position, character compact, recent resonance picks). The
//! assembler renders each conditional line only when its data is
//! present, so callers that have nothing to add (tests, legacy paths)
//! pass `PromptContext::default()` and get the M2-shape system block.

use super::loader::PromptLibrary;
use crate::orchestrator::arc::ArcPosition;
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

/// Phase 6 — optional context fields that fatten the system block when
/// available. The orchestrator fills these from its `SceneSnapshot` +
/// `CharacterRegistry` + `RabbitStore`. Tests and the M2 legacy paths
/// can pass `PromptContext::default()` and get the pre-Phase-6 shape.
#[derive(Debug, Clone, Default)]
pub struct PromptContext<'a> {
    /// Scene name (display, for the manuscript-context line).
    pub scene_name: Option<&'a str>,
    /// Arc position bucket. Pair with `scene_ordering / total` upstream;
    /// the assembler renders the labelled bucket only.
    pub arc_position: Option<ArcPosition>,
    /// 0-indexed scene ordering. Rendered as `(#N of M)` when paired
    /// with `manuscript_scene_count`.
    pub scene_ordering: Option<u32>,
    /// Total scenes in the manuscript.
    pub manuscript_scene_count: Option<u32>,
    /// POV character display name, if the scene has one.
    pub pov_character_name: Option<&'a str>,
    /// Primary location name + brief, when present.
    pub location_name: Option<&'a str>,
    pub location_brief: Option<&'a str>,
    /// `character_compact` output for the speaker, when the speaker is
    /// a character. None for personas.
    pub character_compact: Option<&'a str>,
    /// Up to N most-recent resonant rabbit-thought messages, newest
    /// first. Pass `&[]` to omit. The assembler renders them as a
    /// short labelled list.
    pub recent_resonance: &'a [String],
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

/// Phase 6 — render the manuscript-context block. Each line is
/// conditional on the relevant `PromptContext` field being set, so the
/// block is empty when the caller had no context to add. Returns the
/// trailing newline so the system block flows naturally regardless of
/// how many lines this contributes.
fn manuscript_context_block(ctx: &PromptContext<'_>) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(name) = ctx.scene_name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            let scene_line = match (ctx.scene_ordering, ctx.manuscript_scene_count) {
                (Some(ord), Some(total)) if total > 0 => {
                    format!("Scene: {trimmed} (#{} of {total})", ord + 1)
                }
                _ => format!("Scene: {trimmed}"),
            };
            lines.push(scene_line);
        }
    }
    if let Some(arc) = ctx.arc_position {
        lines.push(format!("Position in arc: {}", arc.label()));
    }
    if let Some(pov) = ctx.pov_character_name {
        let trimmed = pov.trim();
        if !trimmed.is_empty() {
            lines.push(format!("POV: {trimmed}"));
        }
    }
    if let Some(loc) = ctx.location_name {
        let loc_trim = loc.trim();
        if !loc_trim.is_empty() {
            let line = match ctx.location_brief {
                Some(b) if !b.trim().is_empty() => {
                    format!("Location: {loc_trim} — {}", b.trim())
                }
                _ => format!("Location: {loc_trim}"),
            };
            lines.push(line);
        }
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("[manuscript context]\n{}\n", lines.join("\n"))
    }
}

/// Phase 6 — render the speaker's character compact, if any. Personas
/// pass through with an empty block.
fn character_sheet_block(ctx: &PromptContext<'_>) -> String {
    match ctx.character_compact {
        Some(s) if !s.trim().is_empty() => {
            format!("[character sheet]\n{}\n", s.trim())
        }
        _ => String::new(),
    }
}

/// Phase 6 — render up to N recent resonant thoughts. Comma-numbered
/// for terse signal density.
fn recent_resonance_block(ctx: &PromptContext<'_>) -> String {
    if ctx.recent_resonance.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = Vec::new();
    lines.push("[recent resonance picks]".to_string());
    for (i, msg) in ctx.recent_resonance.iter().enumerate() {
        let trimmed = msg.trim();
        if !trimmed.is_empty() {
            lines.push(format!("{}. {trimmed}", i + 1));
        }
    }
    if lines.len() <= 1 {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

/// Build the shared `system` block for any Level-0/expand/regenerate
/// assembly: tone clauses, then speaker identity, then trigger framing,
/// then any Phase-6 context blocks, then the task instruction.
fn system_block(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    trigger_framing: &str,
    task_instruction: &str,
    ctx: &PromptContext<'_>,
) -> String {
    format!(
        "{}\n[speaker: {}]\n{}\n[trigger: {}]\n{}\n{}{}{}[task]\n{}\n",
        tone_block(lib),
        speaker.display_name(),
        speaker.prompt_fragment(),
        trigger_id,
        trigger_framing,
        manuscript_context_block(ctx),
        character_sheet_block(ctx),
        recent_resonance_block(ctx),
        task_instruction,
    )
}

/// Assemble a Level-0 (single-pill) request.
pub fn assemble_level_0(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    trigger_id: &str,
    scene_excerpt: &str,
    ctx: &PromptContext<'_>,
) -> Result<PromptRequest, String> {
    let trig = lib
        .trigger(trigger_id)
        .ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib
        .task("pill_level_0")
        .ok_or_else(|| "pill_level_0 task missing".to_string())?;
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task.instruction, ctx);
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
    ctx: &PromptContext<'_>,
) -> Result<PromptRequest, String> {
    let trig = lib
        .trigger(trigger_id)
        .ok_or_else(|| format!("unknown trigger {trigger_id}"))?;
    let task = lib
        .task("pill_expand")
        .ok_or_else(|| "pill_expand task missing".to_string())?;
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task.instruction, ctx);
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
    ctx: &PromptContext<'_>,
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
    let system = system_block(lib, speaker, &trig.id, &trig.framing, &task_instruction, ctx);
    let user = format!(
        "[parent observation]\n{parent_pill_text}\n\n[manuscript excerpt]\n{scene_excerpt}"
    );
    Ok(PromptRequest {
        system,
        user,
        expect_json: true,
    })
}

/// System block for rabbit-hole prompts. No trigger framing — the
/// parent thought *is* the framing. Speaker identity + tone +
/// optional manuscript/character/resonance context + task instruction.
fn system_block_no_trigger(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    task_instruction: &str,
    ctx: &PromptContext<'_>,
) -> String {
    format!(
        "{}\n[speaker: {}]\n{}\n{}{}{}[task]\n{}\n",
        tone_block(lib),
        speaker.display_name(),
        speaker.prompt_fragment(),
        manuscript_context_block(ctx),
        character_sheet_block(ctx),
        recent_resonance_block(ctx),
        task_instruction,
    )
}

/// Phase 4 — fan a parent thought into four directional children
/// (closer / wider / opposite / deeper). Used both for the first
/// fan from a freshly-rooted pill and for subsequent same-trigger
/// fans where the parent is still close to the original premise.
pub fn assemble_rabbit_fan_4(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    parent_text: &str,
    scene_excerpt: &str,
    ctx: &PromptContext<'_>,
) -> Result<PromptRequest, String> {
    let task = lib
        .task("rabbit_fan_4")
        .ok_or_else(|| "rabbit_fan_4 task missing".to_string())?;
    let system = system_block_no_trigger(lib, speaker, &task.instruction, ctx);
    let user = format!(
        "[parent thought]\n{parent_text}\n\n[manuscript excerpt]\n{scene_excerpt}"
    );
    Ok(PromptRequest {
        system,
        user,
        expect_json: true,
    })
}

/// Phase 5.8 — one paragraph in, one Editor-voice observation out.
/// Speaker is always the Editor persona; the prompt deliberately
/// keeps the trigger framing empty because the *content* of the
/// paragraph is the trigger. PASS sentinel is the expected most-
/// common return (the rules already caught everything they can).
pub fn assemble_editor_polish(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    paragraph_text: &str,
    ctx: &PromptContext<'_>,
) -> Result<PromptRequest, String> {
    let task = lib
        .task("editor_polish")
        .ok_or_else(|| "editor_polish task missing".to_string())?;
    let system = system_block_no_trigger(lib, speaker, &task.instruction, ctx);
    let user = format!("[paragraph]\n{paragraph_text}");
    Ok(PromptRequest {
        system,
        user,
        expect_json: false,
    })
}

/// Phase 4 — same shape as `assemble_rabbit_fan_4` but uses the
/// `rabbit_deepen_inherit` task, which instructs the model to keep
/// the parent's stance instead of re-fanning from the original
/// premise. Called on every fan after the first.
pub fn assemble_rabbit_deepen_inherit(
    lib: &PromptLibrary,
    speaker: &dyn Speaker,
    parent_text: &str,
    scene_excerpt: &str,
    ctx: &PromptContext<'_>,
) -> Result<PromptRequest, String> {
    let task = lib
        .task("rabbit_deepen_inherit")
        .ok_or_else(|| "rabbit_deepen_inherit task missing".to_string())?;
    let system = system_block_no_trigger(lib, speaker, &task.instruction, ctx);
    let user = format!(
        "[parent thought (already-deepened)]\n{parent_text}\n\n[manuscript excerpt]\n{scene_excerpt}"
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
            &PromptContext::default(),
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
            &PromptContext::default(),
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
            &PromptContext::default(),
        )
        .unwrap();
        assert!(req.system.contains("the rain hesitates"));
        assert!(req.system.contains("a small bell"));
        assert!(req.expect_json);
    }
}
