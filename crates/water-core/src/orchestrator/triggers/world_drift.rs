//! `world_drift` Stage 1 — name+alias scan against the World Bible.
//!
//! For the just-finished paragraph (when the cursor is at a sentence or
//! paragraph boundary), tokenize and look up each word in the
//! [`WorldRegistry`] name+alias index. For each hit, apply the
//! [`crate::world::collision::resolve_token_kind`] policy to skip tokens
//! that resolve to a present character. For surviving hits, require
//! `MIN_CONTEXT_OVERLAP_WORDS` (2) non-stopword overlap between the
//! paragraph and the entry's `[main]` field values — this is the cheap
//! "is this paragraph actually about the entry, vs. just mentioning the
//! name" gate. On pass, emit a `TriggerCandidate` carrying a
//! `requires_confirmation` request rendered from the (Task 17)
//! `world_drift_check` confirmation template.
//!
//! TODO(m4-followup): per-(entry, scene) cooldown is not yet wired.
//! Without it, recurring mentions of the same world entry in one
//! paragraph could fire Stage 1 multiple times. Bounded in practice by
//! the existing FIFO pill eviction (orchestrator/eviction.rs) but
//! sub-optimal. `KNOWN_FRAGILE` #23 (to record at m4 tag time, Task 34):
//! "`world_drift` Stage 1 has no per-(entry, scene) cooldown; recurring
//! mentions can over-fire. Mitigation: pill eviction. Fix: introduce
//! `TriggerHistory` in a follow-up; see `orchestrator/mod.rs` for the
//! integration point sketch."
//!
//! Task 17 ships `prompts/tasks/world_drift_check.toml`. Until then the
//! `render_confirmation_request("world_drift_check", …)` call returns
//! `Err` and the evaluator drops the candidate silently — better silent
//! than a half-built confirmation surface.

use crate::orchestrator::{
    CursorClassification, SpeakerTrack, Trigger, TriggerCandidate, TriggerContext,
};
use crate::world::{collision, WorldEntrySnapshot};

/// Minimum word count for a paragraph to be eligible for world-drift
/// evaluation. Below this, the paragraph is too short for the contextual-
/// overlap gate to be meaningful.
pub const MIN_PARAGRAPH_WORDS: usize = 12;

/// Minimum non-stopword token overlap between the paragraph and an
/// entry's `[main]` field values for Stage 1 to fire. Matches the M4
/// spec § 6.3.
pub const MIN_CONTEXT_OVERLAP_WORDS: usize = 2;

/// Cap on the rendered `entry_excerpt` (the entry's `[main]` block
/// flattened to `key: value` lines) passed to Stage 2. Keeps the
/// confirmation prompt under the budget assumed by `world_drift_check`.
const ENTRY_EXCERPT_MAX_CHARS: usize = 1600;

pub struct WorldDrift;

impl Trigger for WorldDrift {
    fn id(&self) -> &'static str {
        "world_drift"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        let paragraph = ctx.analysis.last_block_text.as_deref()?;
        if word_count(paragraph) < MIN_PARAGRAPH_WORDS {
            return None;
        }

        let paragraph_tokens = tokenize_words(paragraph);

        for token in &paragraph_tokens {
            let lowered = token.to_lowercase();
            let matches = ctx.world_registry.find_by_token(&lowered);
            if matches.is_empty() {
                continue;
            }

            // Apply the collision policy: if the token resolves to a
            // present character (CharacterOnly) or nothing (Neither),
            // skip world-drift firing for this token. WorldOnly /
            // BothFire continue.
            let kind = collision::resolve_token_kind(
                token,
                ctx.characters,
                ctx.world_registry,
                &ctx.scene.characters_present,
            );
            match kind {
                collision::TokenKind::CharacterOnly(_) | collision::TokenKind::Neither => continue,
                collision::TokenKind::WorldOnly(_) | collision::TokenKind::BothFire { .. } => {}
            }

            for entry_id in matches {
                let Some(entry) = ctx.world_registry.by_id(entry_id) else {
                    continue;
                };
                if !has_contextual_overlap(&paragraph_tokens, entry, MIN_CONTEXT_OVERLAP_WORDS) {
                    continue;
                }

                let excerpt = render_entry_excerpt(entry, ENTRY_EXCERPT_MAX_CHARS);
                // Stage 2 prompt ships in Task 17. Until then this is
                // `Err` and we drop the candidate silently — matches
                // the character_dissonance.rs precedent (`.ok()?`).
                let req = ctx
                    .prompts
                    .render_confirmation_request(
                        "world_drift_check",
                        &[
                            ("entry_name", entry.name.as_str()),
                            ("segment_slug", entry.segment_slug.as_str()),
                            ("entry_excerpt", excerpt.as_str()),
                            ("paragraph", paragraph),
                            ("matched_token", token.as_str()),
                        ],
                    )
                    .ok()?;

                return Some(TriggerCandidate {
                    trigger_id: self.id().to_string(),
                    priority: 5.5,
                    preferred_track: SpeakerTrack::Persona,
                    reason: format!("possible contradiction with {}", entry.name),
                    block_target_id: Some(ctx.telemetry.block_id.clone()),
                    requires_confirmation: Some(req),
                });
            }
        }

        None
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Tokenize on non-alphanumeric boundaries, preserving in-word apostrophes
/// (`don't`, `Pell's`). Empties dropped. Case preserved — callers
/// lowercase as needed for registry lookups.
#[must_use]
pub fn tokenize_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Returns true when at least `min` distinct non-stopword tokens from
/// `paragraph_tokens` (length-≥3, lowercased) appear in any string value
/// under the entry's `[main]` section. Cheap proxy for "the paragraph is
/// actually about this entry, not just name-dropping."
fn has_contextual_overlap(
    paragraph_tokens: &[String],
    entry: &WorldEntrySnapshot,
    min: usize,
) -> bool {
    let stopwords: std::collections::HashSet<&str> = [
        "the", "a", "an", "of", "to", "in", "on", "at", "by", "for", "with", "and", "or", "but",
        "is", "are", "was", "were", "be", "been", "being", "this", "that", "these", "those", "it",
        "its", "he", "she", "they", "her", "his", "their",
    ]
    .into_iter()
    .collect();

    let Some(entry_main) = entry.data.get("main").and_then(|v| v.as_object()) else {
        return false;
    };
    let mut entry_words: std::collections::HashSet<String> = std::collections::HashSet::new();
    for v in entry_main.values() {
        if let Some(s) = v.as_str() {
            for w in tokenize_words(s) {
                let lw = w.to_lowercase();
                if !stopwords.contains(lw.as_str()) && lw.len() >= 3 {
                    entry_words.insert(lw);
                }
            }
        }
    }
    if entry_words.is_empty() {
        return false;
    }

    let mut overlap = 0usize;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in paragraph_tokens {
        let lt = t.to_lowercase();
        if entry_words.contains(&lt) && seen.insert(lt) {
            overlap += 1;
            if overlap >= min {
                return true;
            }
        }
    }
    false
}

/// Flatten the entry's `[main]` section to a `key: value` block for the
/// Stage-2 confirmation prompt, truncated to `max_chars`. Non-string
/// values are JSON-serialized. Output is stable-ordered by JSON object
/// iteration (insertion order under `serde_json::Map`).
fn render_entry_excerpt(entry: &WorldEntrySnapshot, max_chars: usize) -> String {
    let Some(main) = entry.data.get("main").and_then(|v| v.as_object()) else {
        return String::new();
    };
    let mut out = String::new();
    for (k, v) in main {
        let val_str = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let line = format!("{k}: {val_str}\n");
        if out.len() + line.len() > max_chars {
            // Append as much as fits on a char boundary.
            let remaining = max_chars.saturating_sub(out.len());
            let mut end = remaining.min(line.len());
            while end > 0 && !line.is_char_boundary(end) {
                end -= 1;
            }
            out.push_str(&line[..end]);
            break;
        }
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::world::WorldEntrySnapshot;
    use crate::Id;

    #[test]
    fn word_count_counts_whitespace_words() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("  one  two  three  "), 3);
    }

    #[test]
    fn tokenize_drops_punctuation() {
        let v = tokenize_words("She walked past Pell, then onward.");
        assert_eq!(v, vec!["She", "walked", "past", "Pell", "then", "onward"]);
    }

    fn make_entry(main_fields: serde_json::Value) -> WorldEntrySnapshot {
        let mut data = serde_json::Map::new();
        data.insert("main".to_string(), main_fields);
        WorldEntrySnapshot {
            id: Id::new(),
            segment_id: Id::new(),
            segment_slug: "locations".to_string(),
            name: "Pell".to_string(),
            aliases: vec![],
            data: serde_json::Value::Object(data),
        }
    }

    #[test]
    fn contextual_overlap_returns_true_at_threshold() {
        let entry = make_entry(serde_json::json!({
            "sensory_detail": "Dust thick enough to read fingertips in the sub-basement"
        }));
        let paragraph = tokenize_words("She saw the dust on the fingertips in the sub-basement.");
        assert!(has_contextual_overlap(&paragraph, &entry, 2));
    }

    #[test]
    fn contextual_overlap_returns_false_below_threshold() {
        let entry = make_entry(serde_json::json!({
            "sensory_detail": "Dust thick enough to read fingertips in"
        }));
        let paragraph = tokenize_words("She walked past quickly.");
        assert!(!has_contextual_overlap(&paragraph, &entry, 2));
    }

    #[test]
    fn evaluator_does_not_fire_when_paragraph_too_short() {
        // 11 words — below MIN_PARAGRAPH_WORDS = 12. Never reaches
        // registry lookup, never reaches render_confirmation_request,
        // so this test is independent of Task 17.
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            last_block_text: Some(
                "She walked past quickly and then she turned around.".to_string(),
            ),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 100,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot::default();
        let characters = crate::character::registry::CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            world_registry: test_util::test_world_registry(),
            prompts: test_util::test_prompts(),
        };
        assert!(WorldDrift.evaluate(&ctx).is_none());
    }

    #[test]
    fn evaluator_does_not_fire_mid_sentence() {
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: CursorClassification::MidSentence,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            last_block_text: Some(
                "A long enough paragraph with plenty of words to clear the minimum threshold here."
                    .to_string(),
            ),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 100,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot::default();
        let characters = crate::character::registry::CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            world_registry: test_util::test_world_registry(),
            prompts: test_util::test_prompts(),
        };
        assert!(WorldDrift.evaluate(&ctx).is_none());
    }

    #[test]
    fn evaluator_does_not_fire_when_no_world_entries() {
        // Empty world registry → find_by_token always empty → no fire.
        // Independent of Task 17 because render is never reached.
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            last_block_text: Some(
                "She walked past Pell with measured steps and turned toward the river bank."
                    .to_string(),
            ),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 100,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot::default();
        let characters = crate::character::registry::CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            // Default WorldRegistry is empty.
            world_registry: test_util::test_world_registry(),
            prompts: test_util::test_prompts(),
        };
        assert!(WorldDrift.evaluate(&ctx).is_none());
    }

    #[test]
    fn entry_excerpt_truncates_at_cap() {
        let entry = make_entry(serde_json::json!({
            "long_field": "x".repeat(2000)
        }));
        let out = render_entry_excerpt(&entry, 100);
        assert!(out.len() <= 100, "got {} chars", out.len());
    }

    #[test]
    fn evaluator_fires_when_paragraph_overlaps_with_entry() {
        // M4 Task 17 integration: with `prompts/tasks/world_drift_check.toml`
        // now shipping, the evaluator's `.ok()?` call no longer silently
        // drops; a full Stage-1 candidate (including the rendered Stage-2
        // `ConfirmationRequest`) is emitted.
        //
        // Recipe (mirrors `WorldRegistry::from_db` happy-path tests):
        //   1. Open in-memory DB + tempdir for project root.
        //   2. Seed the 6 built-in world segments for a fresh project.
        //   3. Create an entry "Pell" under `locations` with a
        //      `main.sensory_detail` whose vocabulary overlaps the
        //      paragraph (≥ MIN_CONTEXT_OVERLAP_WORDS = 2 non-stopword
        //      tokens of length ≥ 3).
        //   4. Build a `WorldRegistry::from_db` snapshot.
        //   5. Run the evaluator at a paragraph boundary (cursor != mid-
        //      sentence) with a paragraph that names "Pell" and shares the
        //      overlap vocabulary.
        //   6. Assert: candidate emitted, trigger_id == "world_drift",
        //      requires_confirmation populated with kind ==
        //      "world_drift_check" and the entry's name in the user prompt.
        use crate::world::WorldStore;
        use crate::{Db, ProjectStore};

        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let project = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&project.id).unwrap();
        let locations = store
            .find_segment_by_slug(&project.id, "locations")
            .unwrap()
            .expect("locations segment must exist after seed_builtins");
        // Seed `main.sensory_detail` so the overlap gate finds shared
        // vocabulary in the paragraph below.
        store
            .create_entry_seeded(
                &locations.id,
                "Pell",
                "main.sensory_detail",
                "Dust thick enough to read fingertips in the sub-basement",
            )
            .unwrap();
        let world_reg =
            crate::world::WorldRegistry::from_db(&db, &project.id, dir.path().to_path_buf())
                .unwrap();

        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        // Paragraph: ≥ MIN_PARAGRAPH_WORDS (12) words, names "Pell", and
        // shares "dust", "fingertips", "sub", "basement" with the entry's
        // sensory_detail — well above the 2-word overlap floor.
        let analysis = AnalysisSnapshot {
            last_block_text: Some(
                "She walked into Pell and saw the dust on her fingertips in the sub-basement."
                    .to_string(),
            ),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 100,
            seconds_since_last_pill: 60,
        };
        let project_snap = ProjectSnapshot::default();
        let characters = crate::character::registry::CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project_snap,
            characters: &characters,
            world_registry: &world_reg,
            prompts: test_util::test_prompts(),
        };

        let cand = WorldDrift
            .evaluate(&ctx)
            .expect("expected candidate when paragraph overlaps with entry");
        assert_eq!(cand.trigger_id, "world_drift");
        assert_eq!(cand.preferred_track, SpeakerTrack::Persona);
        let req = cand
            .requires_confirmation
            .as_ref()
            .expect("Task 17 prompt landed; render_confirmation_request must succeed");
        assert_eq!(req.kind, "world_drift_check");
        assert!(
            req.user.contains("Pell"),
            "user prompt must include entry name; got: {}",
            req.user
        );
        assert!(
            !req.user.contains("{{"),
            "no {{ placeholders should remain after substitution; got: {}",
            req.user
        );
    }
}
