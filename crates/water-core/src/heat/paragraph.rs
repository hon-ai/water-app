//! Paragraph-level body partitioning + text hashing.
//!
//! Every metric in the heat module is indexed by `paragraph_ix`, so the
//! orchestrator + compute paths share one definition of "what is a
//! paragraph." This module owns that definition: paragraphs are split
//! on runs of two-or-more newlines, matching CommonMark's loose
//! paragraph break. Leading + trailing whitespace within a paragraph
//! is preserved (writers may use a leading space deliberately).
//!
//! The text-hash returned here is a 16-char hex digest of the
//! paragraph's content; used by `HeatStore` as the cache key for
//! deciding whether a cached row's metric is still valid.

/// One paragraph of body text + its content hash. Indexed positionally
/// in the returned vector: the value at index `i` is paragraph `i`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph<'a> {
    /// Byte offset (inclusive) where this paragraph starts in the
    /// original body string. The renderer uses this for the "click
    /// strip → scroll editor" navigation in Phase E.
    pub byte_offset: usize,
    /// The paragraph text, exclusive of the trailing blank-line(s)
    /// that delimit it from the next paragraph.
    pub text: &'a str,
    /// 16-char lowercase-hex digest of `text`. Stable across processes
    /// (SHA-style truncated). Used as the heat-cache key.
    pub text_hash: String,
}

/// Partition `body` into paragraphs, returning each paragraph's offset,
/// text slice, and content hash. Empty body returns an empty vec;
/// trailing newlines alone do NOT create a phantom empty paragraph.
#[must_use]
pub fn partition(body: &str) -> Vec<Paragraph<'_>> {
    if body.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // Look for a paragraph-break sequence: \n followed by one-or-more
        // additional whitespace-only-newline characters. The minimum
        // break is "\n\n" (two newlines = blank line between paragraphs).
        if bytes[i] == b'\n' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            // Slice the paragraph (start..i), trim leading newlines from
            // the slice if any survived a previous break.
            let raw = &body[start..i];
            push_paragraph(&mut out, start, raw);
            // Skip the run of \n that delimits this paragraph from the
            // next — paragraphs separated by three+ newlines should
            // still yield exactly one break.
            i += 2;
            while i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
            start = i;
            continue;
        }
        i += 1;
    }
    // Trailing paragraph (no trailing blank line).
    if start < bytes.len() {
        let raw = &body[start..];
        push_paragraph(&mut out, start, raw);
    }
    out
}

fn push_paragraph<'a>(out: &mut Vec<Paragraph<'a>>, offset: usize, text: &'a str) {
    // Skip slices that are wholly whitespace — they're a residual of
    // double-newline runs, not a real paragraph.
    if text.trim().is_empty() {
        return;
    }
    out.push(Paragraph {
        byte_offset: offset,
        text,
        text_hash: hash_text(text),
    });
}

/// Stable, deterministic 16-char lowercase-hex content digest. Uses the
/// FNV-1a 64-bit hash so the implementation is tiny + zero-dep — the
/// goal isn't cryptographic strength, just "paragraph text changed,
/// invalidate the cache row."
#[must_use]
pub fn hash_text(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_empty_body_returns_empty_vec() {
        assert!(partition("").is_empty());
    }

    #[test]
    fn partition_single_paragraph() {
        let body = "She crossed into the sub-basement.";
        let p = partition(body);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].byte_offset, 0);
        assert_eq!(p[0].text, body);
    }

    #[test]
    fn partition_splits_on_double_newline() {
        let body = "First para.\n\nSecond para.\n\nThird para.";
        let p = partition(body);
        assert_eq!(p.len(), 3);
        assert_eq!(p[0].text, "First para.");
        assert_eq!(p[1].text, "Second para.");
        assert_eq!(p[2].text, "Third para.");
        // Offsets are byte-accurate.
        assert_eq!(p[0].byte_offset, 0);
        assert_eq!(&body[p[1].byte_offset..p[1].byte_offset + p[1].text.len()], "Second para.");
    }

    #[test]
    fn partition_collapses_runs_of_blank_lines() {
        let body = "One\n\n\n\nTwo";
        let p = partition(body);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text, "One");
        assert_eq!(p[1].text, "Two");
    }

    #[test]
    fn partition_drops_whitespace_only_paragraphs() {
        let body = "Real para.\n\n   \n\nAnother real para.";
        let p = partition(body);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text, "Real para.");
        assert_eq!(p[1].text, "Another real para.");
    }

    #[test]
    fn partition_preserves_internal_single_newlines() {
        let body = "line one\nline two\n\nnext para";
        let p = partition(body);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].text, "line one\nline two");
        assert_eq!(p[1].text, "next para");
    }

    #[test]
    fn hash_text_is_deterministic() {
        assert_eq!(hash_text("hello"), hash_text("hello"));
        assert_ne!(hash_text("hello"), hash_text("hellp"));
    }

    #[test]
    fn hash_text_returns_16_hex_chars() {
        let h = hash_text("anything at all");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(h.chars().all(|c| !c.is_ascii_uppercase()));
    }

    #[test]
    fn partition_stamps_unique_hashes_per_paragraph_when_text_differs() {
        let body = "Alpha.\n\nBeta.";
        let p = partition(body);
        assert_ne!(p[0].text_hash, p[1].text_hash);
    }

    #[test]
    fn partition_stamps_identical_hashes_for_identical_text() {
        // Two copies of the same paragraph at different offsets must
        // yield the same hash — hash is content-only.
        let body = "Echo.\n\nEcho.";
        let p = partition(body);
        assert_eq!(p[0].text_hash, p[1].text_hash);
    }
}
