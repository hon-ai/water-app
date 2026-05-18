//! Regex-driven tone blacklist filter. Compiles patterns from
//! `tone.toml::blacklist_regex.patterns` and drops any pill text that
//! matches at least one pattern.

use super::{FilterDecision, PostFilter};
use regex::Regex;

/// Pre-compiled regex filter. Constructed once at startup (via
/// [`ToneBlacklistFilter::compile`]) and shared across the orchestrator.
pub struct ToneBlacklistFilter {
    /// Pairs of (raw source pattern, compiled regex). The raw pattern is
    /// retained so `Drop` reasons remain human-readable.
    patterns: Vec<(String, Regex)>,
}

impl ToneBlacklistFilter {
    /// Compile the supplied patterns. Returns an error string identifying
    /// the first pattern that fails to parse — `builtin_post_filters` treats
    /// such a failure as a fatal startup condition.
    pub fn compile(raw: &[String]) -> Result<Self, String> {
        let mut patterns = Vec::with_capacity(raw.len());
        for p in raw {
            let re = Regex::new(p).map_err(|e| format!("invalid tone pattern '{p}': {e}"))?;
            patterns.push((p.clone(), re));
        }
        Ok(Self { patterns })
    }
}

impl PostFilter for ToneBlacklistFilter {
    fn id(&self) -> &'static str {
        "tone_blacklist"
    }

    fn evaluate(&self, pill_text: &str) -> FilterDecision {
        for (raw, re) in &self.patterns {
            if re.is_match(pill_text) {
                return FilterDecision::Drop {
                    reason: format!("matched blacklist pattern: {raw}"),
                };
            }
        }
        FilterDecision::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::PromptLibrary;

    fn filter() -> ToneBlacklistFilter {
        let lib = PromptLibrary::load_builtin().unwrap();
        ToneBlacklistFilter::compile(&lib.tone.blacklist_regex.patterns).unwrap()
    }

    #[test]
    fn drops_you_should() {
        let f = filter();
        assert!(matches!(
            f.evaluate("You should try a different angle."),
            FilterDecision::Drop { .. }
        ));
    }

    #[test]
    fn drops_consider() {
        let f = filter();
        assert!(matches!(
            f.evaluate("Consider rewriting this paragraph."),
            FilterDecision::Drop { .. }
        ));
    }

    #[test]
    fn drops_as_an_ai() {
        let f = filter();
        assert!(matches!(
            f.evaluate("As an AI, I notice the cadence."),
            FilterDecision::Drop { .. }
        ));
    }

    #[test]
    fn drops_this_is_good() {
        let f = filter();
        assert!(matches!(
            f.evaluate("This is good prose."),
            FilterDecision::Drop { .. }
        ));
    }

    #[test]
    fn drops_this_is_bad() {
        let f = filter();
        assert!(matches!(
            f.evaluate("This is bad prose."),
            FilterDecision::Drop { .. }
        ));
    }

    #[test]
    fn passes_clean_pill() {
        let f = filter();
        assert_eq!(
            f.evaluate("Something held at the threshold — not fear, not yet curiosity."),
            FilterDecision::Pass
        );
    }

    #[test]
    fn compile_rejects_invalid_pattern() {
        let bad = vec!["(unclosed".to_string()];
        assert!(ToneBlacklistFilter::compile(&bad).is_err());
    }
}
