//! Kimi (Moonshot AI) `LlmProvider` adapter.
//!
//! Moonshot exposes an OpenAI-compatible Chat Completions API at
//! `https://api.moonshot.ai`. The wire format matches OpenAI's
//! exactly, so the request / response handling mirrors
//! `super::openai`. The reason to keep Kimi as a *separate* adapter
//! rather than a "just point OpenAiProvider at a different base
//! URL" trick:
//!
//!   1. Stable `ProviderId("kimi")` for routing, per-provider
//!      cost tracking, secrets keying, and the renderer's
//!      Settings UI grouping.
//!   2. Distinct health-check model (`moonshot-v1-8k` — the
//!      cheapest available; OpenAI's health uses `gpt-4o-mini`,
//!      which Moonshot doesn't host).
//!   3. Default model picks favor the long-context variants
//!      (`kimi-k2-0905-preview` is 256k context; `moonshot-v1-128k`
//!      is a calm fallback). Long context is the *reason* a writer
//!      would add this provider — Water can embed entire drafts
//!      into a single prompt instead of stitching excerpts.
//!
//! The bouquet path delegates to the same `build_user_with_exclusions`
//! + `parse_bouquet_json` helpers OpenAI uses; the JSON shape is
//! identical.

use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default model used when a single-shot `generate_raw` call doesn't
/// specify one. Picked to match what the renderer's curated picker
/// shows as the recommended default — Kimi K2 has the longest context
/// (256k) on Moonshot's catalog.
pub const KIMI_DEFAULT_MODEL: &str = "kimi-k2-0905-preview";

/// Default base URL — Moonshot's international endpoint. The China
/// endpoint (`https://api.moonshot.cn`) is reachable from inside the
/// PRC; constructed via `with_base_url` when needed.
pub const KIMI_DEFAULT_BASE_URL: &str = "https://api.moonshot.ai";

/// Cheapest model on Moonshot's catalog; used by `health()` so a
/// connectivity probe doesn't burn the larger tiers' rate budget.
pub const KIMI_HEALTH_MODEL: &str = "moonshot-v1-8k";

pub struct KimiProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl KimiProvider {
    /// Create a provider pointed at the public Moonshot endpoint.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build,
    /// which should not happen with the static configuration used
    /// here.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, KIMI_DEFAULT_BASE_URL)
    }

    /// Create a provider with a custom base URL. Used by tests
    /// (wiremock) and by writers routing through a regional
    /// endpoint or self-hosted gateway.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build.
    #[must_use]
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest"),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageOut,
}

#[derive(Deserialize)]
struct ChatMessageOut {
    content: String,
}

#[async_trait]
impl LlmProvider for KimiProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("kimi")
    }

    async fn health(&self) -> Result<()> {
        let body = ChatRequest {
            model: KIMI_HEALTH_MODEL,
            temperature: 0.0,
            max_tokens: 1,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "Respond with the single character A.",
                },
                ChatMessage {
                    role: "user",
                    content: "ping",
                },
            ],
        };
        let r = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("kimi health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("kimi health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(
            &req.user,
            &req.previous_variants_first_words,
            req.n_variants,
        );
        let body = ChatRequest {
            model: &req.model,
            temperature: req.temperature,
            max_tokens: req.max_output_tokens,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: &req.system,
                },
                ChatMessage {
                    role: "user",
                    content: &user,
                },
            ],
        };
        let r = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("kimi: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("kimi http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("kimi json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("kimi: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    /// Single-shot text generation. Used by level-0 pill dispatch,
    /// rabbit-hole fan, and editor polish — every path that calls
    /// `LlmRouter::generate_raw_with_default`. Without this override
    /// the trait's default returned an error and Kimi silently
    /// dropped every pill the orchestrator tried to emit.
    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            KIMI_DEFAULT_MODEL
        } else {
            &req.model
        };
        // Moonshot's API rejects `max_tokens=0`; clamp to a sensible
        // default that fits the 22-word pill cap with margin.
        let max_tokens = if req.max_output_tokens == 0 {
            512
        } else {
            req.max_output_tokens
        };
        let body = ChatRequest {
            model,
            temperature: req.temperature,
            max_tokens,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: &req.system,
                },
                ChatMessage {
                    role: "user",
                    content: &req.user,
                },
            ],
        };
        let r = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("kimi raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("kimi raw http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("kimi raw json: {e}")))?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("kimi raw: no choices".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn id_is_stable() {
        let p = KimiProvider::new("test");
        assert_eq!(p.id().as_str(), "kimi");
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer kk-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "content": "[{\"angle\":\"feel\",\"text\":\"x\"},{\"angle\":\"notice\",\"text\":\"y\"},{\"angle\":\"wonder\",\"text\":\"z\"}]"
                    }
                }]
            })))
            .mount(&server)
            .await;
        let p = KimiProvider::with_base_url("kk-secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "kimi-k2-0905-preview".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "z");
    }

    #[tokio::test]
    async fn health_uses_cheap_model() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "A"}}]
            })))
            .mount(&server)
            .await;
        let p = KimiProvider::with_base_url("kk-secret", server.uri());
        p.health().await.unwrap();
        // The first request body should use the cheap health model.
        let received = server.received_requests().await.unwrap();
        let body = received[0].body_json::<serde_json::Value>().unwrap();
        assert_eq!(body["model"], KIMI_HEALTH_MODEL);
    }
}
