//! `PostFilter` trait + built-in filters.
//!
//! M2 Task 18 ships [`tone_blacklist::ToneBlacklistFilter`], driven by the
//! regex patterns in `prompts/tone.toml::blacklist_regex.patterns`. The
//! orchestrator (downstream tasks) runs every generated pill through the
//! filter chain before surfacing it to the editor; a `Drop` decision causes
//! the pill to be discarded with the supplied reason logged.

pub mod tone_blacklist;

/// The outcome of a single filter applied to a pill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    /// The pill is acceptable as far as this filter is concerned.
    Pass,
    /// The pill must be discarded. `reason` is a short human-readable
    /// explanation used for logs/diagnostics.
    Drop { reason: String },
}

/// A post-generation pill filter.
///
/// Filters are pure and stateless: given the pill text they emit a
/// [`FilterDecision`]. The orchestrator stops at the first `Drop`.
pub trait PostFilter: Send + Sync {
    /// Stable identifier for logging (e.g. `"tone_blacklist"`).
    fn id(&self) -> &'static str;

    /// Evaluate `pill_text`. The text is the final user-visible string;
    /// trim/normalization happens upstream.
    fn evaluate(&self, pill_text: &str) -> FilterDecision;
}

/// Build the default built-in filter chain. M2 ships exactly one filter
/// (`ToneBlacklistFilter`); future tasks may append more.
///
/// # Panics
/// Panics if any pattern in `tone_patterns` fails to compile as a regex.
/// All built-in patterns ship from `tone.toml` and are validated by the
/// `ToneBlacklistFilter::compile` tests; a panic here indicates a corrupt
/// `tone.toml`, which is a fatal startup condition.
#[must_use]
pub fn builtin_post_filters(tone_patterns: &[String]) -> Vec<Box<dyn PostFilter>> {
    vec![Box::new(
        tone_blacklist::ToneBlacklistFilter::compile(tone_patterns)
            .expect("built-in tone patterns must compile"),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::PromptLibrary;

    #[test]
    fn builtin_chain_has_tone_blacklist() {
        let lib = PromptLibrary::load_builtin().unwrap();
        let chain = builtin_post_filters(&lib.tone.blacklist_regex.patterns);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id(), "tone_blacklist");
    }
}
