//! Phase 5 — rule-based diagnostic engine.
//!
//! Pure functions: take a block of prose, return findings. No DB,
//! no IO. The caller (`EditorPillStore`) persists findings as
//! editor_pill rows and emits them to the renderer.
//!
//! Rules implemented in v1 (UX_SPEC §E.2):
//!   - passive_voice
//!   - adverb_density
//!   - repetition
//!   - dialog_tag_overuse
//!   - common_mistake
//!
//! Deferred (need extra plumbing not justified by the first writer
//! pass): weak_verb (needs adjective lexicon), sentence_length_variance
//! (cross-sentence stats), spelling (embedded dict).

use regex::Regex;
use std::sync::OnceLock;

/// The taxonomy of rules. Stored as `rule TEXT` in `editor_pill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorRule {
    PassiveVoice,
    AdverbDensity,
    Repetition,
    DialogTagOveruse,
    CommonMistake,
    /// Phase 5.7 — `to-be` auxiliary + adjective-suffixed word where a
    /// stronger verb would carry the sentence (e.g. "was loud" →
    /// "thundered"). Suffix-based detection — no adjective lexicon.
    WeakVerb,
    /// Phase 5.7 — five consecutive sentences whose word counts fall
    /// within ±3. Reads as a metronome on the page.
    SentenceLengthVariance,
    /// Phase 5.8 — LLM-polish observation. The rule layer can't
    /// generate these; the model surfaces one observation per
    /// modified paragraph that the rules can't see (metaphor
    /// recurrence, image collision, etc.). Persisted alongside the
    /// rule-based rows so the diagnostics tab + underline pipeline
    /// treat them uniformly.
    EditorPolish,
}

impl EditorRule {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EditorRule::PassiveVoice => "passive_voice",
            EditorRule::AdverbDensity => "adverb_density",
            EditorRule::Repetition => "repetition",
            EditorRule::DialogTagOveruse => "dialog_tag_overuse",
            EditorRule::CommonMistake => "common_mistake",
            EditorRule::WeakVerb => "weak_verb",
            EditorRule::SentenceLengthVariance => "sentence_length_variance",
            EditorRule::EditorPolish => "editor_polish",
        }
    }
}

/// Severity bucket — drives the underline hue + filter UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSeverity {
    Observation,
    Suggestion,
    Warning,
}

impl EditorSeverity {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EditorSeverity::Observation => "observation",
            EditorSeverity::Suggestion => "suggestion",
            EditorSeverity::Warning => "warning",
        }
    }
}

/// A single diagnostic hit inside one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticFinding {
    pub rule: EditorRule,
    pub severity: EditorSeverity,
    /// Char offset (start, exclusive end) inside the block's text.
    pub start: usize,
    pub end: usize,
    /// The exact 3-10 word phrase the underline will land on. Caller
    /// uses this as the `text_snippet` anchor field.
    pub snippet: String,
    /// Optional concrete replacement (e.g., spelling correction or
    /// common-mistake fix). `None` for stylistic observations.
    pub suggestion: Option<String>,
    /// Extra payload — for repetition we carry the offending word
    /// + occurrence count. Phrasebank renders this into the message.
    pub detail: Option<String>,
}

/// Run every enabled rule on a block of prose. Findings come back in
/// rule-then-start order so the underline plugin can de-dupe
/// overlapping ranges deterministically.
#[must_use]
pub fn run_diagnostics_on_block(text: &str) -> Vec<DiagnosticFinding> {
    let mut out: Vec<DiagnosticFinding> = Vec::new();
    out.extend(scan_passive_voice(text));
    out.extend(scan_adverb_density(text));
    out.extend(scan_repetition(text));
    out.extend(scan_dialog_tag_overuse(text));
    out.extend(scan_common_mistake(text));
    out.extend(scan_weak_verb(text));
    out.extend(scan_sentence_length_variance(text));
    out
}

// ─────────────────────── passive_voice ───────────────────────
fn passive_voice_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // "auxiliary verb of being" + space + word ending in `-ed`.
        // Ignores -ed words like "fed" / "fled" by requiring at least
        // three characters before the `ed` (a hand-tuned heuristic
        // that catches the common shapes — *opened*, *finished*,
        // *believed* — without flagging short irregulars).
        Regex::new(r"(?i)\b(was|were|been|is|are|be|am|being)\s+(\w{3,}ed)\b")
            .expect("static regex")
    })
}

fn scan_passive_voice(text: &str) -> Vec<DiagnosticFinding> {
    let mut out = Vec::new();
    for m in passive_voice_re().captures_iter(text) {
        let full = m.get(0).unwrap();
        out.push(DiagnosticFinding {
            rule: EditorRule::PassiveVoice,
            severity: EditorSeverity::Suggestion,
            start: full.start(),
            end: full.end(),
            snippet: full.as_str().to_string(),
            suggestion: None,
            detail: None,
        });
    }
    out
}

// ─────────────────────── adverb_density ───────────────────────
fn ly_word_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(\w{4,}ly)\b").expect("static regex"))
}

/// > 2 `-ly` adverbs per 100 words → fire one finding anchored at the
/// 3rd offender in the window. We scan the block as one 100-word
/// window for v1; long blocks are rare and the rendered underline
/// reads fine on a per-cluster anchor.
fn scan_adverb_density(text: &str) -> Vec<DiagnosticFinding> {
    let word_count = text.split_whitespace().count();
    if word_count == 0 {
        return Vec::new();
    }
    let matches: Vec<_> = ly_word_re().find_iter(text).collect();
    if matches.is_empty() {
        return Vec::new();
    }
    // Density = adverbs * (100 / words). > 2 → fire.
    #[allow(clippy::cast_precision_loss)]
    let density = matches.len() as f32 * 100.0 / word_count as f32;
    if density <= 2.0 || matches.len() < 3 {
        return Vec::new();
    }
    // Anchor at the third adverb so the writer's eye lands inside
    // the cluster, not at the first one (which often reads fine in
    // isolation).
    let pivot = &matches[2];
    let count = matches.len();
    vec![DiagnosticFinding {
        rule: EditorRule::AdverbDensity,
        severity: EditorSeverity::Suggestion,
        start: pivot.start(),
        end: pivot.end(),
        snippet: pivot.as_str().to_string(),
        suggestion: None,
        detail: Some(format!("{count} -ly in {word_count} words")),
    }]
}

// ─────────────────────── repetition ───────────────────────
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "if", "then", "of", "in", "on", "at",
    "to", "for", "with", "from", "by", "as", "is", "are", "was", "were", "be",
    "been", "being", "am", "do", "does", "did", "have", "has", "had", "i",
    "you", "he", "she", "it", "we", "they", "me", "him", "her", "us", "them",
    "my", "your", "his", "its", "our", "their", "this", "that", "these",
    "those", "what", "which", "who", "whom", "whose", "where", "when", "why",
    "how", "not", "no", "yes", "so", "very", "just", "than", "more", "most",
    "less", "least", "some", "any", "all", "each", "every", "other", "another",
    "such", "would", "could", "should", "will", "shall", "can", "may", "might",
];

fn is_stopword(w: &str) -> bool {
    STOPWORDS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(w))
}

/// Same non-stopword ≥ 4 times in the block → fire one finding at
/// the FIRST occurrence so the underline lands on the head of the
/// chime. Carries the word + count in `detail`.
fn scan_repetition(text: &str) -> Vec<DiagnosticFinding> {
    use std::collections::HashMap;
    // Walk char indices so we can recover offsets cheaply.
    let mut counts: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut start: Option<usize> = None;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let is_word = (b.is_ascii_alphanumeric() || b == b'\'') && b != b'\n';
        if is_word {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            let word = &text[s..i];
            let lc = word.to_ascii_lowercase();
            if word.len() >= 3 && !is_stopword(word) {
                counts.entry(lc).or_default().push((s, i));
            }
        }
        i += 1;
    }
    if let Some(s) = start {
        let word = &text[s..text.len()];
        let lc = word.to_ascii_lowercase();
        if word.len() >= 3 && !is_stopword(word) {
            counts.entry(lc).or_default().push((s, text.len()));
        }
    }
    let mut out = Vec::new();
    for (word, occurrences) in counts {
        if occurrences.len() < 4 {
            continue;
        }
        let (start, end) = occurrences[0];
        out.push(DiagnosticFinding {
            rule: EditorRule::Repetition,
            severity: EditorSeverity::Suggestion,
            start,
            end,
            snippet: text[start..end].to_string(),
            suggestion: None,
            detail: Some(format!(
                "'{word}' × {n}",
                n = occurrences.len()
            )),
        });
    }
    out
}

// ─────────────────────── dialog_tag_overuse ───────────────────────
fn dialog_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)\bsaid\s+(\w{3,}ly)\b"#).expect("static regex")
    })
}

fn scan_dialog_tag_overuse(text: &str) -> Vec<DiagnosticFinding> {
    let mut out = Vec::new();
    for m in dialog_tag_re().captures_iter(text) {
        let full = m.get(0).unwrap();
        let adverb = m.get(1).map(|x| x.as_str().to_string());
        out.push(DiagnosticFinding {
            rule: EditorRule::DialogTagOveruse,
            severity: EditorSeverity::Suggestion,
            start: full.start(),
            end: full.end(),
            snippet: full.as_str().to_string(),
            suggestion: None,
            detail: adverb,
        });
    }
    out
}

// ─────────────────────── common_mistake ───────────────────────
/// `(pattern, replacement)` pairs the writer almost certainly meant
/// to type as `replacement`. Case-insensitive match; the surfaced
/// suggestion is rendered back in the writer's casing.
const COMMON_MISTAKES: &[(&str, &str)] = &[
    ("could of", "could have"),
    ("would of", "would have"),
    ("should of", "should have"),
    ("might of", "might have"),
    ("must of", "must have"),
    ("their is", "there is"),
    ("their are", "there are"),
    ("there own", "their own"),
    ("its own way of", "its own way of"), // placeholder for editorialization later
    ("alot", "a lot"),
    ("its'", "its"),
    ("youre", "you're"),
    ("dont", "don't"),
    ("wont", "won't"),
    ("cant", "can't"),
];

fn scan_common_mistake(text: &str) -> Vec<DiagnosticFinding> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for (pat, repl) in COMMON_MISTAKES {
        // Treat the lookup like a fixed-string substring search;
        // the regex crate compiles the literal as a fast DFA but
        // we can do this even cheaper with `str::find`.
        let mut from = 0;
        while let Some(rel) = lower[from..].find(pat) {
            let abs = from + rel;
            let end = abs + pat.len();
            // Word-boundary guard: only fire when the match isn't
            // wedged between letters (e.g. "their is" shouldn't fire
            // inside "wherethe[ir is]tand"). Cheap: look at the
            // char immediately before / after.
            let before_ok = abs == 0
                || !text[..abs]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
            let after_ok = end >= text.len()
                || !text[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric());
            if before_ok && after_ok {
                out.push(DiagnosticFinding {
                    rule: EditorRule::CommonMistake,
                    severity: EditorSeverity::Warning,
                    start: abs,
                    end,
                    snippet: text[abs..end].to_string(),
                    suggestion: Some((*repl).to_string()),
                    detail: None,
                });
            }
            from = end;
        }
    }
    out
}

// ─────────────────────── weak_verb ───────────────────────
fn weak_verb_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Auxiliary "to-be" + a word with a common adjective suffix.
        // Suffix list is the load-bearing heuristic: avoids needing
        // an adjective lexicon while catching the typical "was sad",
        // "is loud", "were beautiful" shapes. Suffix gates also catch
        // some non-adjectives (e.g. "ferry", "boundary") — accepted
        // false-positive rate for v1; future weak_verb_v2 can lex.
        // Word length ≥ 4 keeps "was a" / "is on" out.
        Regex::new(
            r"(?i)\b(was|were|is|are|be|am|being)\s+(\w{3,}(?:ful|ous|ive|al|ic|able|ible|ant|ent|y|ly))\b",
        )
        .expect("static regex")
    })
}

/// Stoplist of adjective-suffixed words that *aren't* the weak-verb
/// shape — common functional words ("very", "only", "really") that
/// would be false positives under the suffix-only gate.
fn weak_verb_skip(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "very" | "only" | "really" | "rarely" | "lately" | "fully" | "early"
            | "every" | "any" | "many" | "carry" | "marry" | "tally"
    )
}

fn scan_weak_verb(text: &str) -> Vec<DiagnosticFinding> {
    let mut out = Vec::new();
    for cap in weak_verb_re().captures_iter(text) {
        let full = cap.get(0).unwrap();
        let word = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        if weak_verb_skip(word) {
            continue;
        }
        out.push(DiagnosticFinding {
            rule: EditorRule::WeakVerb,
            severity: EditorSeverity::Suggestion,
            start: full.start(),
            end: full.end(),
            snippet: full.as_str().to_string(),
            suggestion: None,
            detail: Some(word.to_string()),
        });
    }
    out
}

// ─────────────────────── sentence_length_variance ───────────────────────
/// Split a block of prose into sentences. Terminator: `. ` / `! ` /
/// `? ` / end-of-block. Returns each sentence with its byte-range
/// inside the block so a finding can anchor at the cadence cluster.
fn iter_sentences(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut start = 0_usize;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if (b == b'.' || b == b'!' || b == b'?')
            && (i + 1 >= bytes.len() || bytes[i + 1].is_ascii_whitespace())
        {
            let end = i + 1;
            // Skip empty / whitespace-only sentences.
            let slice = &text[start..end];
            if slice.split_whitespace().next().is_some() {
                out.push((start, end));
            }
            // Advance start past the punctuation + any whitespace.
            let mut j = end;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            start = j;
            i = j;
            continue;
        }
        i += 1;
    }
    if start < text.len() {
        let slice = &text[start..text.len()];
        if slice.split_whitespace().next().is_some() {
            out.push((start, text.len()));
        }
    }
    out
}

const VARIANCE_WINDOW: usize = 5;
const VARIANCE_SPREAD: usize = 3;

/// Fire when any 5 consecutive sentences have word counts whose
/// max-min spread is ≤ 3. Anchors at the first sentence in the
/// offending window.
fn scan_sentence_length_variance(text: &str) -> Vec<DiagnosticFinding> {
    let sentences = iter_sentences(text);
    if sentences.len() < VARIANCE_WINDOW {
        return Vec::new();
    }
    let counts: Vec<usize> = sentences
        .iter()
        .map(|(s, e)| text[*s..*e].split_whitespace().count())
        .collect();
    let mut findings = Vec::new();
    let mut last_window_start: Option<usize> = None;
    for i in 0..=counts.len() - VARIANCE_WINDOW {
        let window = &counts[i..i + VARIANCE_WINDOW];
        let min = *window.iter().min().unwrap_or(&0);
        let max = *window.iter().max().unwrap_or(&0);
        if max.saturating_sub(min) <= VARIANCE_SPREAD && min > 0 {
            // Don't fire two findings whose windows overlap — once
            // the writer sees one cadence flag for a run, anchoring
            // a second one a sentence over is noise.
            if let Some(prev) = last_window_start {
                if i < prev + VARIANCE_WINDOW {
                    continue;
                }
            }
            let (start, end) = sentences[i];
            findings.push(DiagnosticFinding {
                rule: EditorRule::SentenceLengthVariance,
                severity: EditorSeverity::Observation,
                start,
                end,
                snippet: text[start..end].to_string(),
                suggestion: None,
                detail: Some(format!("{min}-{max} words across {VARIANCE_WINDOW}")),
            });
            last_window_start = Some(i);
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_of(findings: &[DiagnosticFinding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.rule.as_str()).collect()
    }

    #[test]
    fn passive_voice_fires_on_being_plus_finished() {
        let findings = scan_passive_voice("The chapter was finished at midnight.");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].snippet.to_ascii_lowercase().contains("finished"));
    }

    #[test]
    fn passive_voice_no_fire_on_active_voice() {
        let findings = scan_passive_voice("She walked across the square.");
        assert!(findings.is_empty());
    }

    #[test]
    fn adverb_density_fires_when_three_lys_in_short_block() {
        let findings = scan_adverb_density(
            "He spoke softly, walked quickly, and looked carefully.",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, EditorRule::AdverbDensity);
    }

    #[test]
    fn adverb_density_no_fire_when_below_threshold() {
        let findings = scan_adverb_density("He spoke softly across the long table.");
        assert!(findings.is_empty());
    }

    #[test]
    fn repetition_fires_on_four_uses_of_same_word() {
        let findings =
            scan_repetition("hand on hand again hand and hand fell open.");
        assert!(
            !findings.is_empty(),
            "expected fire on 'hand' × 4; got none"
        );
        let kinds = rules_of(&findings);
        assert!(kinds.contains(&"repetition"));
    }

    #[test]
    fn repetition_skips_stopwords() {
        // "the" appears many times but is a stopword — no fire.
        let findings = scan_repetition("the cat sat the mat the road the rain the sky.");
        assert!(findings.iter().all(|f| !f.snippet.eq_ignore_ascii_case("the")));
    }

    #[test]
    fn dialog_tag_overuse_fires_on_said_plus_adverb() {
        let findings = scan_dialog_tag_overuse(r#""leave," he said quickly."#);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].snippet.to_ascii_lowercase().contains("quickly"));
    }

    #[test]
    fn dialog_tag_no_fire_when_no_adverb() {
        let findings = scan_dialog_tag_overuse(r#""leave," he said and turned away."#);
        assert!(findings.is_empty());
    }

    #[test]
    fn common_mistake_fires_on_could_of() {
        let findings = scan_common_mistake("He could of stayed.");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].suggestion.as_deref(), Some("could have"));
    }

    #[test]
    fn common_mistake_respects_word_boundaries() {
        // The pattern "alot" is inside "alottery" — must NOT fire.
        let findings = scan_common_mistake("The alottery sign blinked.");
        assert!(findings.is_empty());
    }

    #[test]
    fn run_diagnostics_orchestrates_multiple_rules() {
        let text = "She was finished. He could of left. He said quickly.";
        let findings = run_diagnostics_on_block(text);
        let rules = rules_of(&findings);
        assert!(rules.contains(&"passive_voice"));
        assert!(rules.contains(&"common_mistake"));
        assert!(rules.contains(&"dialog_tag_overuse"));
    }

    #[test]
    fn snippet_text_matches_block_substring() {
        // The text_snippet field must equal the exact slice
        // [start..end) of the block — the anchor resolver depends
        // on this.
        let text = "She was finished at last.";
        let findings = scan_passive_voice(text);
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert_eq!(&text[f.start..f.end], f.snippet);
    }

    // ── Phase 5.7 weak_verb tests ──

    #[test]
    fn weak_verb_fires_on_was_plus_adjective_suffix() {
        let findings = scan_weak_verb("The room was beautiful.");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, EditorRule::WeakVerb);
        assert!(findings[0].snippet.to_ascii_lowercase().contains("beautiful"));
    }

    #[test]
    fn weak_verb_fires_on_is_plus_dangerous() {
        let findings = scan_weak_verb("The road is dangerous.");
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn weak_verb_skips_functional_words() {
        // "is very" / "was only" must not fire — these adjective-
        // suffixed words are NOT the weak-verb shape.
        let findings = scan_weak_verb("It was very dark. He is only nine.");
        assert!(findings.is_empty(), "expected no weak_verb fires; got {findings:?}");
    }

    #[test]
    fn weak_verb_no_fire_on_short_word() {
        // "was on" → 3rd token is two chars; below the suffix gate.
        let findings = scan_weak_verb("She was on the porch.");
        assert!(findings.is_empty());
    }

    // ── Phase 5.7 sentence_length_variance tests ──

    #[test]
    fn variance_fires_on_five_consecutive_same_length_sentences() {
        // Each sentence ~6 words → max-min spread 0 across 5 → fire.
        let text =
            "She walked through the wet street. He turned to look at her. \
             A bell rang somewhere behind them. The air smelled of distant rain. \
             Their hands moved closer to touch.";
        let findings = scan_sentence_length_variance(text);
        assert_eq!(findings.len(), 1, "expected single variance fire");
        assert_eq!(findings[0].rule, EditorRule::SentenceLengthVariance);
    }

    #[test]
    fn variance_no_fire_when_lengths_vary() {
        let text =
            "She walked. He turned to look at her with the longest pause. \
             A bell rang. The air smelled of distant and forgotten rain. \
             Their hands touched.";
        let findings = scan_sentence_length_variance(text);
        assert!(findings.is_empty(), "expected no fires; got {findings:?}");
    }

    #[test]
    fn variance_no_fire_when_fewer_than_five_sentences() {
        let text = "First. Second. Third. Fourth.";
        let findings = scan_sentence_length_variance(text);
        assert!(findings.is_empty());
    }

    #[test]
    fn variance_does_not_double_fire_on_overlapping_windows() {
        // Eight equal-length sentences: window 0..5 fires, the
        // 1..6, 2..7 windows must be suppressed; only 3..8 may
        // also fire (non-overlap with first).
        let text = "She walked again now. \
                    He stood there for awhile. \
                    A bell rang quietly nearby. \
                    The smell of damp paper. \
                    Their hands were almost touching. \
                    A breath went out softly. \
                    The lamp dimmed for the moment. \
                    They were close and watching.";
        let findings = scan_sentence_length_variance(text);
        // Allow either 1 or 2 fires depending on exact word counts;
        // what matters is we don't get 4+ overlapping anchors.
        assert!(
            findings.len() <= 2,
            "variance should not double-fire on overlap; got {findings:?}"
        );
    }
}
