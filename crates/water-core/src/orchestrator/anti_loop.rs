//! Anti-loop overlap check. Jaccard similarity on stopword-stripped,
//! suffix-stripped tokens. Per-speaker threshold (default 0.70).
//!
//! See `docs/superpowers/specs/2026-05-17-m2-editor-pill-engine.md` § 6.5.

use std::collections::HashSet;
use std::hash::BuildHasher;

const STOPWORDS_RAW: &str = include_str!("../../data/stopwords-en.txt");

fn stopwords() -> HashSet<&'static str> {
    STOPWORDS_RAW.lines().filter(|l| !l.is_empty()).collect()
}

const SUFFIXES: &[&str] = &["ing", "ed", "es", "ly", "s"];

fn strip_suffix(word: &str) -> &str {
    for suf in SUFFIXES {
        if word.len() > suf.len() + 2 && word.ends_with(suf) {
            return &word[..word.len() - suf.len()];
        }
    }
    word
}

#[must_use]
pub fn tokenize(text: &str) -> HashSet<String> {
    let sw = stopwords();
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && !sw.contains(s))
        .map(strip_suffix)
        .map(std::string::ToString::to_string)
        .collect()
}

#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn jaccard<S: BuildHasher>(a: &HashSet<String, S>, b: &HashSet<String, S>) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count();
    let union = a.union(b).count();
    inter as f32 / union as f32
}

/// Returns the max Jaccard overlap of `new_text` against any prior in
/// `prior_texts`. 0.0 if priors is empty.
#[must_use]
pub fn max_overlap(new_text: &str, prior_texts: &[String]) -> f32 {
    let new_toks = tokenize(new_text);
    prior_texts
        .iter()
        .map(|prior| jaccard(&new_toks, &tokenize(prior)))
        .fold(0.0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_strings_have_overlap_one() {
        let a = tokenize("the writer walks softly");
        let b = tokenize("the writer walks softly");
        assert!((jaccard(&a, &b) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stopwords_dropped() {
        let toks = tokenize("the and the");
        assert!(toks.is_empty());
    }

    #[test]
    fn suffix_stripped_collisions() {
        let a = tokenize("walked");
        let b = tokenize("walking");
        let c = tokenize("walks");
        assert!((jaccard(&a, &b) - 1.0).abs() < 1e-5);
        assert!((jaccard(&a, &c) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disjoint_content_is_zero() {
        let a = tokenize("rain falls gently");
        let b = tokenize("mountain breathes cold");
        assert!(jaccard(&a, &b).abs() < 1e-5);
    }

    #[test]
    fn max_overlap_returns_largest() {
        let priors = vec![
            "the rain falls gently".to_string(),
            "mountain breathes cold".to_string(),
        ];
        let new = "rain falls gently on quiet stone";
        let overlap = max_overlap(new, &priors);
        assert!(overlap > 0.4 && overlap < 0.8, "got {overlap}");
    }
}
