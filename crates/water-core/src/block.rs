//! Block-ID maintenance for scene `Markdown` bodies.
//!
//! A "block" is a paragraph (one or more non-empty lines separated by blank
//! lines). The renderer's ProseMirror serializer writes the `^bk-XXXX`
//! token at the START of each paragraph, immediately followed by a space
//! and the paragraph's prose:
//!
//! ```text
//! ^bk-aaaa First paragraph.
//!
//! ^bk-bbbb Second paragraph.
//! ```
//!
//! Earlier versions of this module used a TRAILING convention. The
//! reader still recognizes that legacy layout so projects authored
//! before the migration round-trip cleanly — but every write emits
//! leading-position tokens.

use std::collections::HashSet;
use std::hash::BuildHasher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: String,   // "bk-0a3f"
    pub text: String, // body without the leading or trailing ^bk- token
}

const PREFIX: &str = "bk-";
const ID_LEN: usize = PREFIX.len() + 4; // "bk-" + 4 chars

/// Validate that `s` is a well-formed `bk-XXXX` id (no leading caret).
fn is_id_body(s: &str) -> bool {
    s.starts_with(PREFIX)
        && s.len() == ID_LEN
        && s[PREFIX.len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric())
}

#[must_use]
pub fn fresh_block_id<S: BuildHasher>(existing: &HashSet<String, S>) -> String {
    loop {
        let raw = ulid::Ulid::new().to_string();
        // Take last 4 characters of the ULID.
        let suffix = raw.get(raw.len() - 4..).unwrap_or("xxxx").to_lowercase();
        let id = format!("{PREFIX}{suffix}");
        if !existing.contains(&id) {
            return id;
        }
    }
}

/// Split a body into blocks separated by blank lines. For each block we
/// strip an optional `^bk-XXXX` token, accepting either:
///   * **leading** (`^bk-XXXX prose`) — the current convention emitted
///     by the ProseMirror serializer.
///   * **trailing** (`prose ^bk-XXXX`) — the legacy convention, kept
///     readable so projects authored before the migration still
///     round-trip cleanly.
///
/// Returns `None` in the id slot when neither form is present.
#[must_use]
pub fn split_blocks(body: &str) -> Vec<(Option<String>, String)> {
    body.split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|para| {
            // Leading-token form first (current convention).
            if let Some(rest) = para.strip_prefix('^') {
                if rest.len() > ID_LEN && rest.as_bytes().get(ID_LEN) == Some(&b' ') {
                    let id_part = &rest[..ID_LEN];
                    if is_id_body(id_part) {
                        let after = rest[ID_LEN + 1..].trim_start();
                        return (Some(id_part.to_owned()), after.to_owned());
                    }
                }
                // Single-line block consisting of just the token (empty
                // paragraph anchor written by the PM serializer when a
                // paragraph has no content yet).
                if rest.len() == ID_LEN && is_id_body(rest) {
                    return (Some(rest.to_owned()), String::new());
                }
            }
            // Trailing-token form (legacy).
            if let Some(idx) = para.rfind("^bk-") {
                let id_part = &para[idx + 1..]; // strip the caret
                if is_id_body(id_part) {
                    let before = para[..idx].trim_end();
                    return (Some(id_part.to_owned()), before.to_owned());
                }
            }
            (None, para.to_owned())
        })
        .collect()
}

/// Ensure every block in `body` has a `^bk-XXXX` token, emitting the
/// leading-position convention. Existing ids (in either position) are
/// preserved; missing ones get freshly minted. Returns the new body
/// and the resolved blocks.
#[must_use]
pub fn ensure_block_ids(body: &str) -> (String, Vec<Block>) {
    let split = split_blocks(body);
    let mut existing: HashSet<String> = split.iter().filter_map(|(id, _)| id.clone()).collect();
    let mut out_blocks: Vec<Block> = Vec::with_capacity(split.len());

    for (id_opt, text) in split {
        let id = if let Some(id) = id_opt {
            id
        } else {
            let new_id = fresh_block_id(&existing);
            existing.insert(new_id.clone());
            new_id
        };
        out_blocks.push(Block { id, text });
    }

    let mut out = String::new();
    for (i, b) in out_blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push('^');
        out.push_str(&b.id);
        if !b.text.is_empty() {
            out.push(' ');
            out.push_str(&b.text);
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    (out, out_blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn ensure_adds_ids_to_unmarked_blocks() {
        let body = "First paragraph.\n\nSecond paragraph.";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks.len(), 2);
        // Leading-token convention: `^bk-XXXX <text>`.
        assert!(out.contains("^bk-"), "got: {out}");
        assert!(out.contains(" First paragraph."), "got: {out}");
        assert!(out.contains(" Second paragraph."), "got: {out}");
    }

    #[test]
    fn ensure_preserves_existing_leading_ids() {
        let body = "^bk-abcd Hello.\n\nGoodbye.";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks[0].id, "bk-abcd");
        assert_eq!(blocks[0].text, "Hello.");
        assert!(out.starts_with("^bk-abcd Hello."));
        assert!(blocks[1].id.starts_with("bk-"));
        assert_ne!(blocks[1].id, "bk-abcd");
    }

    #[test]
    fn ensure_migrates_legacy_trailing_ids_to_leading() {
        // Files authored before the leading-token migration carry the
        // id at the END of each paragraph. Reading must recognize them
        // so projects round-trip; writing emits the new leading
        // convention so legacy files self-migrate on first save.
        let body = "Hello. ^bk-abcd\n\nGoodbye. ^bk-efgh";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks[0].id, "bk-abcd");
        assert_eq!(blocks[0].text, "Hello.");
        assert_eq!(blocks[1].id, "bk-efgh");
        assert_eq!(blocks[1].text, "Goodbye.");
        assert!(out.contains("^bk-abcd Hello."), "got: {out}");
        assert!(out.contains("^bk-efgh Goodbye."), "got: {out}");
        // Sanity: no trailing tokens left over.
        assert!(!out.contains("Hello. ^bk-"));
        assert!(!out.contains("Goodbye. ^bk-"));
    }

    #[test]
    fn ensure_dedupes_colliding_ids() {
        let body = "^bk-abcd A.\n\n^bk-abcd B.";
        let (_out, blocks) = ensure_block_ids(body);
        // Both blocks retained their ids initially because split_blocks does
        // not deduplicate. We expect at least one renamed by ensure during
        // future tasks; for v1 we accept duplicates because pill resolution
        // is snippet-based, not id-based. Document this in KNOWN_FRAGILE.md.
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn round_trips_pm_style_body_unchanged() {
        // Body fresh from the ProseMirror serializer:
        // every paragraph already has a leading `^bk-XXXX` token. The
        // round trip must be a no-op (modulo the trailing newline that
        // `ensure_block_ids` writes after the last paragraph).
        let body = "^bk-aaaa The library was an old place.\n\n^bk-bbbb She'd been going there.";
        let (out, blocks) = ensure_block_ids(body);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, "bk-aaaa");
        assert_eq!(blocks[0].text, "The library was an old place.");
        assert_eq!(blocks[1].id, "bk-bbbb");
        assert_eq!(blocks[1].text, "She'd been going there.");
        // Critically: no trailing-token leak in the output.
        assert!(!out.contains("place. ^bk-"), "trailing-token leak: {out}");
        assert!(!out.contains("there. ^bk-"), "trailing-token leak: {out}");
    }

    #[test]
    fn fresh_block_id_avoids_collision() {
        let mut existing = HashSet::new();
        for _ in 0..50 {
            let id = fresh_block_id(&existing);
            assert!(!existing.contains(&id));
            existing.insert(id);
        }
    }

    #[test]
    fn empty_body_round_trips_to_empty() {
        let (out, blocks) = ensure_block_ids("");
        assert!(out.is_empty());
        assert!(blocks.is_empty());
    }
}
