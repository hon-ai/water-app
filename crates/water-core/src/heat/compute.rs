//! Pure-function metric computers. The orchestrator calls these against
//! a scene's body + typing history + per-project registries; the result
//! lands in `HeatStore`.
//!
//! Phase A ships pacing, presence, and world-refs (all local-only).
//! Phase B adds the LLM-backed valence + coherence.

use crate::heat::paragraph::Paragraph;
use regex::{escape, RegexBuilder};

/// One scene_typing_history row, as the compute path consumes it.
/// Decoupled from the storage layer so tests can hand-build fixtures
/// without an SQLite round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypingEvent {
    pub ts_ms: i64,
    pub word_delta: i32,
}

/// One entity (character or world-entry) the compute path counts
/// mentions of: the canonical name plus any aliases. The renderer
/// surfaces this via the Presence and WorldRefs metrics; the
/// case_sensitive flag distinguishes character (M3, case-sensitive
/// on word boundary) from world (M4, case-insensitive).
#[derive(Debug, Clone)]
pub struct Entity {
    /// All matchable names — the canonical name + every alias. Empty
    /// or whitespace-only entries are silently ignored (an artifact
    /// of M4's relaxed alias entry).
    pub names: Vec<String>,
}

/// Compute pacing scores per paragraph from a scene's typing history.
///
/// Approach: bin the typing history into `paragraph_count` temporal
/// buckets. The bucket boundaries split the (first_ts .. last_ts) span
/// uniformly, so paragraph `i` covers the segment of writing time
/// `[first + i*chunk, first + (i+1)*chunk]`. Each bucket's raw score is
/// `sum(word_delta) / chunk_seconds`; the full vector is then min-max
/// normalized to 0.0 .. 1.0 so the renderer can plot it without
/// re-scaling per metric.
///
/// Returns an empty vec when `paragraph_count == 0` or `history` is
/// empty. Returns a uniform `0.5` track when the history spans less
/// than one second (not enough resolution to differentiate buckets).
#[must_use]
pub fn compute_pacing(history: &[TypingEvent], paragraph_count: u32) -> Vec<f32> {
    if paragraph_count == 0 || history.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<TypingEvent> = history.to_vec();
    sorted.sort_by_key(|e| e.ts_ms);
    let first = sorted.first().expect("non-empty checked above").ts_ms;
    let last = sorted.last().expect("non-empty checked above").ts_ms;
    let span_ms = last - first;
    if span_ms < 1000 {
        // Sub-second total session; not enough resolution. Return
        // uniform mid-track so the renderer still draws something.
        return vec![0.5; paragraph_count as usize];
    }
    let chunk_ms = span_ms / i64::from(paragraph_count);
    let mut raw = vec![0.0f32; paragraph_count as usize];
    for ev in &sorted {
        if ev.word_delta <= 0 {
            continue;
        }
        let offset = ev.ts_ms - first;
        // chunk_ms > 0 because span_ms >= 1000 and paragraph_count <= span_ms.
        let mut ix = (offset / chunk_ms) as usize;
        if ix >= raw.len() {
            ix = raw.len() - 1;
        }
        raw[ix] += ev.word_delta as f32;
    }
    // Normalize words per chunk into words-per-second per chunk.
    let chunk_s = chunk_ms as f32 / 1000.0;
    for r in &mut raw {
        *r /= chunk_s;
    }
    // Min-max into 0..=1. If every bucket has the same value (e.g.
    // perfectly even pacing), collapse to 0.5.
    let max = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min = raw.iter().copied().fold(f32::INFINITY, f32::min);
    if (max - min).abs() < f32::EPSILON {
        return vec![0.5; paragraph_count as usize];
    }
    raw.iter().map(|r| (r - min) / (max - min)).collect()
}

/// Count distinct entity mentions per paragraph, normalized 0..=1 by
/// scene-max. `case_sensitive=true` (character-style); `false` for
/// world-style. The compute path uses this for both Presence and
/// WorldRefs — wrappers below configure the case-sensitivity flag.
///
/// "Distinct" means: a paragraph that mentions Marcus three times
/// scores the same as one that mentions Marcus once. We count
/// entities-that-appear, not raw matches — the intent is the strip
/// shows "thickness of cast" rather than "name repetition." A
/// paragraph mentioning Marcus + Talia scores 2; one with just
/// Marcus scores 1.
#[must_use]
pub fn compute_entity_mentions(
    paragraphs: &[Paragraph<'_>],
    entities: &[Entity],
    case_sensitive: bool,
) -> Vec<f32> {
    if paragraphs.is_empty() {
        return Vec::new();
    }
    // Pre-build one regex per entity (union of their non-empty names),
    // so we don't re-compile per paragraph. Skip entities whose names
    // are all empty.
    let patterns: Vec<Option<regex::Regex>> = entities
        .iter()
        .map(|e| build_entity_regex(&e.names, case_sensitive))
        .collect();

    let raw: Vec<f32> = paragraphs
        .iter()
        .map(|p| {
            let mut hits = 0u32;
            for re in patterns.iter().flatten() {
                if re.is_match(p.text) {
                    hits += 1;
                }
            }
            hits as f32
        })
        .collect();

    let max = raw.iter().copied().fold(0.0f32, f32::max);
    if max < 0.5 {
        // No paragraph has any mentions; uniform zero track.
        return vec![0.0; paragraphs.len()];
    }
    raw.iter().map(|r| r / max).collect()
}

/// Compile a single regex matching any of `names` at word boundaries.
/// Returns `None` if every name is empty/whitespace-only.
fn build_entity_regex(names: &[String], case_sensitive: bool) -> Option<regex::Regex> {
    let alts: Vec<String> = names
        .iter()
        .filter(|n| !n.trim().is_empty())
        .map(|n| escape(n))
        .collect();
    if alts.is_empty() {
        return None;
    }
    let pattern = format!(r"\b(?:{})\b", alts.join("|"));
    RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(ts_ms: i64, word_delta: i32) -> TypingEvent {
        TypingEvent { ts_ms, word_delta }
    }

    #[test]
    fn empty_history_returns_empty_vec() {
        assert!(compute_pacing(&[], 5).is_empty());
    }

    #[test]
    fn zero_paragraphs_returns_empty_vec() {
        let history = [ev(0, 5), ev(2000, 5)];
        assert!(compute_pacing(&history, 0).is_empty());
    }

    #[test]
    fn sub_second_span_returns_uniform_half_track() {
        let history = [ev(0, 3), ev(500, 4)];
        let result = compute_pacing(&history, 4);
        assert_eq!(result, vec![0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn perfectly_even_pacing_collapses_to_half_track() {
        // Equal word_delta across uniformly-spaced events maps to equal
        // raw scores per bucket, which the min-max step collapses to 0.5.
        let history: Vec<TypingEvent> = (0..10).map(|i| ev(i * 1000, 5)).collect();
        let result = compute_pacing(&history, 5);
        for r in &result {
            assert!((r - 0.5).abs() < 1e-5);
        }
    }

    #[test]
    fn early_burst_maps_to_high_score_in_first_bucket() {
        // Front-loaded: 20 words in the first second, then nothing for 9s.
        let history = [
            ev(0, 10),
            ev(500, 10),
            ev(10_000, 0),
        ];
        let result = compute_pacing(&history, 5);
        assert_eq!(result.len(), 5);
        // Bucket 0 should be the max; remaining buckets stay near 0.
        assert!((result[0] - 1.0).abs() < 1e-5, "got {result:?}");
        for r in &result[1..] {
            assert!(*r < 0.1, "expected near-zero late buckets, got {result:?}");
        }
    }

    #[test]
    fn negative_word_deltas_are_ignored() {
        // Backspacing (word_delta < 0) shouldn't add to pacing; pacing
        // measures the forward stride of the writing, not the churn.
        let history = [ev(0, 10), ev(2000, -10), ev(4000, 10)];
        let result = compute_pacing(&history, 3);
        // Bucket 1 (the all-negative one) should be at the minimum (0.0
        // after normalization); buckets 0 and 2 should equal at the max.
        assert!((result[1] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn unsorted_history_still_works() {
        // Same as front-loaded test but with events shuffled.
        let history = [ev(10_000, 0), ev(500, 10), ev(0, 10)];
        let result = compute_pacing(&history, 5);
        assert!((result[0] - 1.0).abs() < 1e-5);
    }

    // ---- Entity-mention tests (Presence / WorldRefs) ----

    use crate::heat::paragraph::partition;

    fn entity(names: &[&str]) -> Entity {
        Entity {
            names: names.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn mentions_empty_paragraphs_returns_empty_vec() {
        let result = compute_entity_mentions(&[], &[entity(&["Marcus"])], true);
        assert!(result.is_empty());
    }

    #[test]
    fn mentions_no_entities_returns_zero_track() {
        let body = "First.\n\nSecond.\n\nThird.";
        let paras = partition(body);
        let result = compute_entity_mentions(&paras, &[], true);
        assert_eq!(result, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn mentions_zero_when_no_paragraph_has_a_hit() {
        let body = "Nothing here.\n\nOr here.";
        let paras = partition(body);
        let result = compute_entity_mentions(&paras, &[entity(&["Marcus"])], true);
        assert_eq!(result, vec![0.0, 0.0]);
    }

    #[test]
    fn mentions_case_sensitive_for_characters() {
        // Character names match by case + word boundary.
        let body = "Marcus walked in.\n\nmarcus is lowercase.";
        let paras = partition(body);
        let result = compute_entity_mentions(&paras, &[entity(&["Marcus"])], true);
        // Paragraph 0 has one hit (Marcus), 1 has none ("marcus" != "Marcus").
        assert_eq!(result.len(), 2);
        assert!((result[0] - 1.0).abs() < 1e-5);
        assert!((result[1] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn mentions_case_insensitive_for_world_entries() {
        // World entries match regardless of case.
        let body = "The Library is here.\n\nshe entered the library.";
        let paras = partition(body);
        let result =
            compute_entity_mentions(&paras, &[entity(&["The Library"])], false);
        // Both paragraphs match; normalized both = 1.0.
        assert!((result[0] - 1.0).abs() < 1e-5);
        assert!((result[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mentions_word_boundary_anchored() {
        // "Mark" shouldn't match inside "Marketing" or "Marks".
        let body = "Mark arrived.\n\nMarketing department.\n\nMarks on paper.";
        let paras = partition(body);
        let result = compute_entity_mentions(&paras, &[entity(&["Mark"])], true);
        assert!((result[0] - 1.0).abs() < 1e-5);
        assert!((result[1] - 0.0).abs() < 1e-5);
        assert!((result[2] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn mentions_distinct_per_entity_not_per_occurrence() {
        // Two paragraphs: one mentions Marcus three times, one mentions
        // Marcus + Talia once each. The second scores higher
        // (two distinct entities > one entity).
        let body =
            "Marcus said. Marcus said again. Marcus said once more.\n\nMarcus and Talia.";
        let paras = partition(body);
        let result = compute_entity_mentions(
            &paras,
            &[entity(&["Marcus"]), entity(&["Talia"])],
            true,
        );
        assert_eq!(result.len(), 2);
        // P0: 1 distinct entity. P1: 2 distinct entities. Normalized:
        // P0 = 0.5, P1 = 1.0.
        assert!((result[0] - 0.5).abs() < 1e-5);
        assert!((result[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mentions_alias_matches_count_for_the_same_entity() {
        // One entity ("Marcus" + alias "Vale") mentioned by both
        // names in the same paragraph still counts as one entity.
        let body = "Marcus walked. Vale watched.\n\nVale walked.";
        let paras = partition(body);
        let result = compute_entity_mentions(
            &paras,
            &[entity(&["Marcus", "Vale"])],
            true,
        );
        // Both paragraphs trip the same single entity; normalized 1.0.
        assert!((result[0] - 1.0).abs() < 1e-5);
        assert!((result[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mentions_empty_alias_strings_are_ignored() {
        // Defensive: a stray empty-string alias must not match every
        // word boundary in the body.
        let body = "Anything.\n\nElse.";
        let paras = partition(body);
        let result =
            compute_entity_mentions(&paras, &[entity(&["", "  "])], true);
        // Empty/whitespace-only names → no regex built → no matches.
        assert_eq!(result, vec![0.0, 0.0]);
    }
}
