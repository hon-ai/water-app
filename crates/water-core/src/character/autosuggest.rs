//! Scene-character autosuggest: scans a scene body for character full names
//! and aliases (case-sensitive, `\b` word boundary) and returns the top 5
//! most-mentioned characters with mention counts.
//!
//! `KNOWN_FRAGILE` #15 (spec § 15): this is name-string-matching, not
//! co-reference resolution. Pronouns ("he", "her") don't link. Manual
//! multi-select in the Scene Metadata sheet bridges the gap.
//!
//! Match policy:
//! * Case-sensitive — `"marcus"` does NOT match `"Marcus"`.
//! * Word-boundary anchored — `"Mark"` does NOT match inside `"Marketing"`.
//! * Empty needles are silently skipped (treated as zero matches), so
//!   stray `""` entries in an `aliases` vec don't blow the count up.
//! * Top 5 by descending count; ties resolve in input order via a
//!   stable sort.
//! * `\b` is Unicode-aware (Rust's `regex` default). A name embedded
//!   inside a longer run of word characters won't match — e.g. for CJK
//!   compounds, `\b李\b` matches a standalone `李` but not `李` inside
//!   `李明`. Same family of limitations as the no-co-ref caveat above.
//!
//! The scanner is fed an [`AutosuggestRow`] — a slim view of the
//! character needed for matching. Production callers build this via
//! [`crate::character::CharacterStore::list_all_with_aliases`], which
//! walks `data_json.main.aliases` once at the store layer so the scanner
//! itself stays JSON-free.

use crate::Id;
use regex::Regex;
use serde::Serialize;

/// Input row for the autosuggest scanner. Slim by design — the scanner
/// only needs identity + names to match against the body text. Tests
/// can construct these directly; production code uses
/// [`crate::character::CharacterStore::list_all_with_aliases`].
#[derive(Debug, Clone)]
pub struct AutosuggestRow {
    pub character_id: Id,
    pub full_name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutosuggestResult {
    pub character_id: Id,
    pub full_name: String,
    pub mention_count: u32,
}

/// Returns the top 5 most-mentioned characters in `body_text`, sorted
/// by descending mention count (stable on ties). Characters with zero
/// mentions are filtered out before truncation.
#[must_use]
pub fn suggest_for_scene_body(
    body_text: &str,
    characters: &[AutosuggestRow],
) -> Vec<AutosuggestResult> {
    let mut results: Vec<AutosuggestResult> = Vec::new();
    for character in characters {
        let mut count = count_word_boundary_matches(body_text, &character.full_name);
        for alias in &character.aliases {
            count = count.saturating_add(count_word_boundary_matches(body_text, alias));
        }
        if count > 0 {
            results.push(AutosuggestResult {
                character_id: character.character_id.clone(),
                full_name: character.full_name.clone(),
                mention_count: count,
            });
        }
    }
    // Stable sort by descending count. Stability preserves input order on
    // ties so tests with deterministic fixtures stay deterministic.
    results.sort_by_key(|r| std::cmp::Reverse(r.mention_count));
    results.truncate(5);
    results
}

/// Count `\b<needle>\b` matches in `haystack`. Empty/whitespace-only
/// needles return 0 — without this, `regex::escape("")` produces a
/// pattern that matches every word boundary in the haystack.
fn count_word_boundary_matches(haystack: &str, needle: &str) -> u32 {
    if needle.trim().is_empty() {
        return 0;
    }
    let pattern = format!(r"\b{}\b", regex::escape(needle));
    let Ok(re) = Regex::new(&pattern) else {
        return 0;
    };
    u32::try_from(re.find_iter(haystack).count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct an [`AutosuggestRow`] for tests. The `_id` parameter is
    /// kept for readability at call sites (e.g. `row("c1", ...)`); the
    /// actual id is generated fresh each call so tests don't share state.
    fn row(_id: &str, full_name: &str, aliases: &[&str]) -> AutosuggestRow {
        AutosuggestRow {
            character_id: Id::new(),
            full_name: full_name.into(),
            aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn full_name_match() {
        let body = "Marcus walked into the bar. Marcus sat down.";
        let rows = vec![row("c1", "Marcus", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].mention_count, 2);
    }

    #[test]
    fn word_boundary_excludes_substring() {
        // "Mark" should not match inside "Marketing".
        let body = "The marketing meeting ran long.";
        let rows = vec![row("c1", "Mark", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(r.is_empty());
    }

    #[test]
    fn case_sensitive() {
        let body = "marcus walked in.";
        let rows = vec![row("c1", "Marcus", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(
            r.is_empty(),
            "lowercase 'marcus' should not match 'Marcus' (case-sensitive policy)"
        );
    }

    #[test]
    fn aliases_counted() {
        // Fixture: full_name "Marcus Vale" (not present as a contiguous
        // phrase), plus three aliases each present once.
        //   "Marc"   -> 1 (matches `\bMarc\b`, does NOT match inside `Marcus`)
        //   "Marcus" -> 1
        //   "Vale"   -> 1
        // Total mention_count = 3. The full_name itself contributes 0
        // because "Marcus Vale" never appears as a contiguous string.
        let body = "Marc walked in. Marcus sat. Vale watched.";
        let rows = vec![row("c1", "Marcus Vale", &["Marc", "Marcus", "Vale"])];
        let r = suggest_for_scene_body(body, &rows);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].mention_count, 3);
    }

    #[test]
    fn ranked_by_count_top_5() {
        // Six characters with strictly descending mention counts in the
        // body. Build the body by repeating each name N times so the
        // counts come out as 6, 5, 4, 3, 2, 1.
        let names = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"];
        let counts = [6, 5, 4, 3, 2, 1];
        let mut body = String::new();
        for (name, &n) in names.iter().zip(counts.iter()) {
            for _ in 0..n {
                body.push_str(name);
                body.push(' ');
            }
        }
        let rows: Vec<AutosuggestRow> = names.iter().map(|n| row("c", n, &[])).collect();
        let r = suggest_for_scene_body(&body, &rows);
        // Truncated to 5 (Foxtrot drops out).
        assert_eq!(r.len(), 5);
        // Descending mention_count.
        let observed: Vec<u32> = r.iter().map(|x| x.mention_count).collect();
        assert_eq!(observed, vec![6, 5, 4, 3, 2]);
        // Names align with the count order.
        let observed_names: Vec<&str> = r.iter().map(|x| x.full_name.as_str()).collect();
        assert_eq!(
            observed_names,
            vec!["Alpha", "Bravo", "Charlie", "Delta", "Echo"]
        );
    }

    #[test]
    fn empty_alias_doesnt_match_everything() {
        // Without the empty-needle guard, `\b\b` matches every word
        // boundary in the haystack and explodes the count. With the
        // guard the character has zero mentions (full_name absent, empty
        // alias filtered) and drops out of the result entirely.
        let body = "some body text";
        let rows = vec![row("c1", "Marcus", &[""])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(r.is_empty());
    }
}
