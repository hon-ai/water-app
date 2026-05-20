//! LLM-backed metric computers (Valence, Coherence).
//!
//! Both functions take an [`LlmRouter`] and a paragraph (plus, for
//! coherence, the preceding paragraphs as context). They issue a
//! single `generate_raw_with_default` call, parse the response into an
//! `f32`, and surface explicit errors when the model misbehaves.
//!
//! Caching by paragraph text_hash is the orchestrator's responsibility
//! — this module only computes when called. The orchestrator looks up
//! the cached row, compares the live paragraph's text_hash, and only
//! calls these functions when the hash differs.

use crate::llm::LlmRouter;
use crate::{Error, Result};
use std::sync::Arc;

const VALENCE_PROMPT: &str = include_str!("../../../../prompts/tasks/heat_valence.toml");
const COHERENCE_PROMPT: &str = include_str!("../../../../prompts/tasks/heat_coherence.toml");

/// Cap on how many preceding paragraphs we feed the coherence model.
/// Three is the sweet spot: enough context to detect non-trivial breaks,
/// short enough to keep the token cost roughly equivalent to valence's
/// per-paragraph call.
const COHERENCE_CONTEXT_WINDOW: usize = 3;

/// Compute the emotional valence of `paragraph` in the range
/// `[-1.0, 1.0]`. Calls `router.generate_raw_with_default` with the
/// `heat_valence` prompt; parses the response as a decimal. Clamps the
/// parsed value into `[-1.0, 1.0]` so a runaway model that returns
/// `"-2"` doesn't poison the cache.
///
/// # Errors
/// - [`Error::Provider`] if the prompt template fails to load (programmer
///   error caught by tests).
/// - [`Error::Provider`] if the model's response can't be parsed as a
///   decimal. The orchestrator catches this and skips caching.
/// - Whatever error the router returns (provider failure, timeout, etc.).
pub async fn compute_valence(router: &Arc<LlmRouter>, paragraph: &str) -> Result<f32> {
    let (system, user) = render_valence_prompt(paragraph)?;
    let raw = router
        .generate_raw_with_default(system, user)
        .await
        .map_err(|e| Error::Provider(format!("valence: {e}")))?;
    parse_valence(&raw)
}

/// Render the heat_valence prompt template with the paragraph substituted.
/// Pulled out so tests can hit the parser without a router.
fn render_valence_prompt(paragraph: &str) -> Result<(String, String)> {
    #[derive(serde::Deserialize)]
    struct File {
        prompt: Prompt,
    }
    #[derive(serde::Deserialize)]
    struct Prompt {
        system: String,
        user: String,
    }
    let parsed: File = toml::from_str(VALENCE_PROMPT)
        .map_err(|e| Error::Provider(format!("valence prompt parse: {e}")))?;
    let user = parsed.prompt.user.replace("{{paragraph}}", paragraph);
    Ok((parsed.prompt.system, user))
}

/// Compute the topical-continuity score of `paragraphs[ix]` against the
/// `COHERENCE_CONTEXT_WINDOW` paragraphs immediately preceding it.
/// Returns `1.0` (perfect continuity) when `ix == 0` — the first
/// paragraph has nothing to compare against and the strip should not
/// register a phantom break at scene start.
///
/// # Errors
/// - [`Error::Provider`] on prompt parse failure, parse failure on the
///   response, or non-finite parsed value.
/// - Whatever the router returns.
pub async fn compute_coherence(
    router: &Arc<LlmRouter>,
    paragraphs: &[&str],
    ix: usize,
) -> Result<f32> {
    if ix == 0 || ix >= paragraphs.len() {
        return Ok(1.0);
    }
    let start = ix.saturating_sub(COHERENCE_CONTEXT_WINDOW);
    let preceding: String = paragraphs[start..ix].join("\n\n");
    let (system, user) = render_coherence_prompt(&preceding, paragraphs[ix])?;
    let raw = router
        .generate_raw_with_default(system, user)
        .await
        .map_err(|e| Error::Provider(format!("coherence: {e}")))?;
    parse_coherence(&raw)
}

fn render_coherence_prompt(preceding: &str, current: &str) -> Result<(String, String)> {
    #[derive(serde::Deserialize)]
    struct File {
        prompt: Prompt,
    }
    #[derive(serde::Deserialize)]
    struct Prompt {
        system: String,
        user: String,
    }
    let parsed: File = toml::from_str(COHERENCE_PROMPT)
        .map_err(|e| Error::Provider(format!("coherence prompt parse: {e}")))?;
    let user = parsed
        .prompt
        .user
        .replace("{{preceding}}", preceding)
        .replace("{{current}}", current);
    Ok((parsed.prompt.system, user))
}

/// Parse the model's response into a clamped `[0.0, 1.0]` coherence.
fn parse_coherence(raw: &str) -> Result<f32> {
    let trimmed = raw.trim();
    let unquoted = trimmed
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim();
    let n: f32 = unquoted
        .parse()
        .map_err(|_| Error::Provider(format!("coherence parse: response was {raw:?}")))?;
    if !n.is_finite() {
        return Err(Error::Provider(format!(
            "coherence parse: non-finite value {n:?}"
        )));
    }
    Ok(n.clamp(0.0, 1.0))
}

/// Parse the model's response into a clamped `[-1.0, 1.0]` valence.
fn parse_valence(raw: &str) -> Result<f32> {
    let trimmed = raw.trim();
    // Strip a single pair of surrounding quotes if the model added them.
    let unquoted = trimmed
        .trim_start_matches('"')
        .trim_end_matches('"')
        .trim();
    let n: f32 = unquoted
        .parse()
        .map_err(|_| Error::Provider(format!("valence parse: response was {raw:?}")))?;
    if !n.is_finite() {
        return Err(Error::Provider(format!(
            "valence parse: non-finite value {n:?}"
        )));
    }
    Ok(n.clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valence_accepts_typical_decimals() {
        assert!((parse_valence("0.5").unwrap() - 0.5).abs() < 1e-5);
        assert!((parse_valence("-0.8").unwrap() + 0.8).abs() < 1e-5);
        assert!((parse_valence("0").unwrap() - 0.0).abs() < 1e-5);
        assert!((parse_valence("1.0").unwrap() - 1.0).abs() < 1e-5);
        assert!((parse_valence("-1.0").unwrap() + 1.0).abs() < 1e-5);
    }

    #[test]
    fn parse_valence_strips_whitespace() {
        assert!((parse_valence("  0.3  \n").unwrap() - 0.3).abs() < 1e-5);
    }

    #[test]
    fn parse_valence_strips_surrounding_quotes() {
        // Some models hedge with quotes despite the prompt asking for raw.
        assert!((parse_valence("\"0.4\"").unwrap() - 0.4).abs() < 1e-5);
    }

    #[test]
    fn parse_valence_clamps_out_of_range() {
        // Defends against a runaway model returning -2 or +3.
        assert!((parse_valence("-2").unwrap() + 1.0).abs() < 1e-5);
        assert!((parse_valence("3").unwrap() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn parse_valence_rejects_non_numeric() {
        assert!(parse_valence("warm and tender").is_err());
        assert!(parse_valence("").is_err());
        assert!(parse_valence("0.5 (warm)").is_err());
    }

    #[test]
    fn parse_valence_rejects_nan_and_infinity() {
        assert!(parse_valence("NaN").is_err());
        assert!(parse_valence("inf").is_err());
    }

    #[test]
    fn render_valence_prompt_substitutes_paragraph() {
        let (system, user) = render_valence_prompt("She crossed into the dust.").unwrap();
        assert!(system.contains("emotional valence"));
        assert!(user.contains("She crossed into the dust."));
        assert!(!user.contains("{{paragraph}}"));
    }

    #[test]
    fn render_valence_prompt_handles_paragraph_with_special_chars() {
        // Newlines, quotes, etc. should land in `user` verbatim.
        let p = "She said \"goodbye\".\n\nThen she left.";
        let (_, user) = render_valence_prompt(p).unwrap();
        assert!(user.contains("\"goodbye\""));
        assert!(user.contains("Then she left."));
    }

    // ---- Coherence ----

    #[test]
    fn parse_coherence_accepts_typical_decimals() {
        assert!((parse_coherence("0.5").unwrap() - 0.5).abs() < 1e-5);
        assert!((parse_coherence("0").unwrap() - 0.0).abs() < 1e-5);
        assert!((parse_coherence("1.0").unwrap() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn parse_coherence_clamps_to_zero_one() {
        // Negative values are illegal for coherence (it's a similarity);
        // out-of-range values clamp.
        assert!((parse_coherence("-0.3").unwrap() - 0.0).abs() < 1e-5);
        assert!((parse_coherence("1.4").unwrap() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn parse_coherence_rejects_non_numeric() {
        assert!(parse_coherence("smooth flow").is_err());
        assert!(parse_coherence("").is_err());
    }

    #[test]
    fn render_coherence_prompt_substitutes_both_blocks() {
        let (system, user) =
            render_coherence_prompt("Para A.", "Para B.").unwrap();
        assert!(system.contains("topical continuity"));
        assert!(user.contains("Para A."));
        assert!(user.contains("Para B."));
        assert!(!user.contains("{{preceding}}"));
        assert!(!user.contains("{{current}}"));
    }

    // For the async function we verify the non-call edge cases without
    // a router. The actual LLM round-trip is exercised in Phase D + the
    // M5 acceptance smoke.

    #[tokio::test]
    async fn coherence_first_paragraph_returns_one_without_calling_llm() {
        // ix == 0 short-circuits; the function returns 1.0 without ever
        // touching the router. Build a router with NO providers so any
        // accidental call would error — the test passes only because we
        // never get there.
        let router = Arc::new(LlmRouter::new(vec![]));
        let paragraphs = vec!["Only paragraph."];
        let result = compute_coherence(&router, &paragraphs, 0).await.unwrap();
        assert!((result - 1.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn coherence_out_of_range_index_returns_one() {
        // Defensive: ix >= len returns 1.0 (caller bug, but don't panic).
        let router = Arc::new(LlmRouter::new(vec![]));
        let paragraphs = vec!["P1.", "P2."];
        let result = compute_coherence(&router, &paragraphs, 9).await.unwrap();
        assert!((result - 1.0).abs() < 1e-5);
    }
}
