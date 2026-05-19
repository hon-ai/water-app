//! Lemma-overlap Jaccard helper for `character_dissonance` Stage 1 gate.
//!
//! Wraps the M2 `anti_loop` tokenize + jaccard helpers with a slightly
//! different threshold convention (dissonance fires above 0.30; anti-loop
//! fires above per-speaker threshold ~0.70).

pub use crate::orchestrator::anti_loop::{jaccard, tokenize};

#[must_use]
pub fn overlap(a: &str, b: &str) -> f32 {
    let ta = tokenize(a);
    let tb = tokenize(b);
    jaccard(&ta, &tb)
}
