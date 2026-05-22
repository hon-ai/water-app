//! Phase 5 — voice-discipline phrasebank (UX_SPEC §E.7).
//!
//! Each rule has 3–5 Editor-voice phrasings. The renderer picks one
//! deterministically per finding so re-runs against the same span
//! produce the same prose (no fatigue from a freshly-rendered message
//! every debounce). Variables (`{word}`, `{count}`, `{adverb}`,
//! `{snippet}`, `{suggestion}`) are substituted from the finding.
//!
//! Each rendered candidate goes through `tone.toml`'s blacklist
//! regex set before surfacing. A hit means the template is broken
//! against the writer-trance discipline; we ship without it rather
//! than past it (spec wording: "better a bad prompt than a silently-
//! dropped variable" — here, better silent than tone-violating).

use crate::editor::diagnostics::{DiagnosticFinding, EditorRule};
use crate::prompts::loader::ToneClauses;
use regex::RegexSet;
use std::sync::OnceLock;

/// Per-rule phrasings. Order matters — the deterministic picker
/// modulos against the count, so reordering shifts which message
/// each finding gets. Append-only to avoid surprising existing
/// findings on a renderer upgrade.
const PHRASINGS_PASSIVE_VOICE: &[&str] = &[
    "the verb is asleep here.",
    "this sentence is being acted upon.",
    "the action waits for something to do it.",
];

const PHRASINGS_ADVERB_DENSITY: &[&str] = &[
    "the prose is over-explaining.",
    "too many -lys for one breath.",
    "the adverbs are doing the verb's job.",
];

const PHRASINGS_REPETITION: &[&str] = &[
    "'{word}' is showing up a lot. {count} times here.",
    "'{word}' has begun to chime.",
    "the word '{word}' wants a rest.",
];

const PHRASINGS_DIALOG_TAG_OVERUSE: &[&str] = &[
    "'{adverb}' is doing the verb's job.",
    "the tag is louder than the line.",
    "said + adverb. the said could rest.",
];

const PHRASINGS_COMMON_MISTAKE: &[&str] = &[
    "'{snippet}' wants to be '{suggestion}'.",
    "small one: '{snippet}' → '{suggestion}'.",
    "'{snippet}' = '{suggestion}'.",
];

const PHRASINGS_WEAK_VERB: &[&str] = &[
    "'was/is + {detail}' could be one stronger verb.",
    "the verb is doing thin work here.",
    "an adjective is propping up a weak verb.",
];

const PHRASINGS_SENTENCE_LENGTH_VARIANCE: &[&str] = &[
    "the cadence is metronomic right now.",
    "five sentences in a row, same length.",
    "the rhythm has flattened.",
];

fn phrasings_for(rule: EditorRule) -> &'static [&'static str] {
    match rule {
        EditorRule::PassiveVoice => PHRASINGS_PASSIVE_VOICE,
        EditorRule::AdverbDensity => PHRASINGS_ADVERB_DENSITY,
        EditorRule::Repetition => PHRASINGS_REPETITION,
        EditorRule::DialogTagOveruse => PHRASINGS_DIALOG_TAG_OVERUSE,
        EditorRule::CommonMistake => PHRASINGS_COMMON_MISTAKE,
        EditorRule::WeakVerb => PHRASINGS_WEAK_VERB,
        EditorRule::SentenceLengthVariance => PHRASINGS_SENTENCE_LENGTH_VARIANCE,
        // EditorPolish messages come straight from the LLM; the
        // rule layer never asks the phrasebank to render one.
        // Returning an empty slice makes pick_phrasing → empty
        // string → render_message returns None, so any accidental
        // call to render_message on a polish finding stays safe.
        EditorRule::EditorPolish => &[],
    }
}

/// Deterministic phrasing pick per finding. Hash of
/// `(rule_id, snippet)` modulo phrasings count. Stable across runs
/// so debounced re-emission against the same span produces the
/// same prose.
fn pick_phrasing<'a>(rule: EditorRule, snippet: &str) -> &'a str {
    let phrasings = phrasings_for(rule);
    if phrasings.is_empty() {
        return "";
    }
    // FNV-1a 32-bit — overkill stability for a 3-5 element bucket
    // but trivially correct.
    let mut h: u32 = 0x811c_9dc5;
    for b in rule.as_str().as_bytes().iter().chain(snippet.as_bytes()) {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    let ix = (h as usize) % phrasings.len();
    phrasings[ix]
}

/// Build the `RegexSet` lazily from the tone clauses. Cached for the
/// process lifetime once the first call materializes it. Invalid
/// patterns are dropped silently — the loader-side `tone.toml` has
/// already validated the file at startup.
fn blacklist_set(tone: &ToneClauses) -> &'static RegexSet {
    static SET: OnceLock<RegexSet> = OnceLock::new();
    SET.get_or_init(|| {
        // Patterns are deliberately case-insensitive on the tone
        // side; we anchor that here so every template render walks
        // the same gauntlet as a generative pill.
        let patterns: Vec<String> = tone
            .blacklist_regex
            .patterns
            .iter()
            .map(|p| format!("(?i){p}"))
            .collect();
        RegexSet::new(patterns).unwrap_or_else(|_| RegexSet::empty())
    })
}

/// Substitute the finding's payload into the picked template.
/// Returns `None` when the rendered message hits any tone-blacklist
/// pattern — calls drop the row instead of surfacing a tone-broken
/// message.
#[must_use]
pub fn render_message(finding: &DiagnosticFinding, tone: &ToneClauses) -> Option<String> {
    let template = pick_phrasing(finding.rule, &finding.snippet);
    if template.is_empty() {
        return None;
    }
    let mut s = template.to_string();
    s = s.replace("{snippet}", &finding.snippet);
    if let Some(sug) = &finding.suggestion {
        s = s.replace("{suggestion}", sug);
    }
    if let Some(detail) = &finding.detail {
        // Always expose the raw detail for templates that use the
        // catch-all `{detail}` placeholder (weak_verb, cadence).
        s = s.replace("{detail}", detail);
        // Repetition: detail is `'word' × N`. Extract `word` + count
        // for the templates that name them.
        if let Some(word) = detail
            .strip_prefix('\'')
            .and_then(|rest| rest.split('\'').next())
        {
            s = s.replace("{word}", word);
        }
        if let Some(count) = detail.rsplit_once(' ').map(|x| x.1) {
            s = s.replace("{count}", count);
        }
        // Dialog tag: detail is the adverb itself.
        if !detail.contains(' ') {
            s = s.replace("{adverb}", detail);
        }
    }
    // Final tone-blacklist gate. A template that survived review may
    // still produce a blacklisted phrasing after substitution (e.g.
    // a writer's snippet contains "you should") — the gate catches
    // that and we ship without it.
    let set = blacklist_set(tone);
    if set.is_match(&s) {
        return None;
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::diagnostics::EditorSeverity;
    use crate::prompts::loader::PromptLibrary;

    fn tone() -> &'static ToneClauses {
        static T: OnceLock<ToneClauses> = OnceLock::new();
        T.get_or_init(|| PromptLibrary::load_builtin().unwrap().tone)
    }

    fn finding(
        rule: EditorRule,
        snippet: &str,
        detail: Option<&str>,
        suggestion: Option<&str>,
    ) -> DiagnosticFinding {
        DiagnosticFinding {
            rule,
            severity: EditorSeverity::Suggestion,
            start: 0,
            end: snippet.len(),
            snippet: snippet.to_string(),
            suggestion: suggestion.map(str::to_string),
            detail: detail.map(str::to_string),
        }
    }

    #[test]
    fn picks_phrasing_deterministically_for_same_snippet() {
        let a = pick_phrasing(EditorRule::PassiveVoice, "was finished");
        let b = pick_phrasing(EditorRule::PassiveVoice, "was finished");
        assert_eq!(a, b);
    }

    #[test]
    fn picks_a_phrasing_from_the_pool() {
        let p = pick_phrasing(EditorRule::PassiveVoice, "was opened");
        assert!(PHRASINGS_PASSIVE_VOICE.contains(&p));
    }

    #[test]
    fn render_passive_voice_emits_nonempty() {
        let f = finding(EditorRule::PassiveVoice, "was finished", None, None);
        let msg = render_message(&f, tone()).expect("expected message");
        assert!(!msg.is_empty());
    }

    #[test]
    fn render_repetition_substitutes_word_and_count() {
        let f = finding(
            EditorRule::Repetition,
            "hand",
            Some("'hand' × 4"),
            None,
        );
        let msg = render_message(&f, tone()).expect("expected message");
        // Either the message names the word + count, or it doesn't
        // use those placeholders (depends on the picked phrasing).
        // What we assert: no `{word}` / `{count}` placeholders
        // survived into the surface string.
        assert!(!msg.contains("{word}"));
        assert!(!msg.contains("{count}"));
        assert!(msg.contains("hand"));
    }

    #[test]
    fn render_common_mistake_substitutes_suggestion() {
        let f = finding(
            EditorRule::CommonMistake,
            "could of",
            None,
            Some("could have"),
        );
        let msg = render_message(&f, tone()).expect("expected message");
        assert!(!msg.contains("{snippet}"));
        assert!(!msg.contains("{suggestion}"));
        assert!(msg.contains("could of"));
        assert!(msg.contains("could have"));
    }

    #[test]
    fn render_drops_when_blacklisted_phrase_in_snippet() {
        // If a writer's snippet contains "you should" (a tone-
        // blacklisted phrase from prompts/tone.toml), the rendered
        // message will too — the gate must drop it.
        let f = finding(
            EditorRule::CommonMistake,
            "you should of",
            None,
            Some("you should have"),
        );
        let msg = render_message(&f, tone());
        assert!(
            msg.is_none(),
            "tone blacklist should drop messages that surface 'you should'"
        );
    }
}
