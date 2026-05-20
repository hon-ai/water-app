//! Pure-function metric computers. The orchestrator calls these against
//! a scene's body + typing history + per-project registries; the result
//! lands in `HeatStore`.
//!
//! Phase A ships pacing (this task) and presence + world-refs (Task 5).
//! Phase B adds the LLM-backed valence + coherence.

/// One scene_typing_history row, as the compute path consumes it.
/// Decoupled from the storage layer so tests can hand-build fixtures
/// without an SQLite round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypingEvent {
    pub ts_ms: i64,
    pub word_delta: i32,
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
}
