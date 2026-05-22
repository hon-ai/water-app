//! Google Gemini `LlmProvider` adapter.
//!
//! Google exposes an OpenAI-compatible Chat Completions endpoint
//! alongside their native Generative Language API. We use the OpenAI-
//! compat path (`/v1beta/openai/chat/completions`) so the wire format
//! matches every other adapter in this module — same auth (Bearer),
//! same request shape, same response shape.
//!
//! Distinct adapter (rather than "point OpenAiProvider at the Gemini
//! base URL"):
//!   1. Stable `ProviderId("gemini")` for routing + Settings UI grouping.
//!   2. Distinct health-check model (`gemini-2.5-flash` — cheap + fast).
//!   3. Default model is `gemini-2.5-flash` for the same reason: writers
//!      adding Gemini want speed/cost first; they can swap to `pro` for
//!      higher-quality pills via the Settings model picker.

use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const GEMINI_DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

/// Cheap fast-tier model used by health probes + the default for
/// new installations. Pro tier is too expensive to call on every
/// health check.
pub const GEMINI_HEALTH_MODEL: &str = "gemini-2.5-flash";
pub const GEMINI_DEFAULT_MODEL: &str = "gemini-2.5-flash";

pub struct GeminiProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl GeminiProvider {
    /// Create a provider pointed at Google's public Generative
    /// Language API in OpenAI-compat mode.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, GEMINI_DEFAULT_BASE_URL)
    }

    /// Create a provider with a custom base URL. Used by tests
    /// (wiremock) and by future proxy / regional-endpoint setups.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build.
    #[must_use]
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
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
impl LlmProvider for GeminiProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("gemini")
    }

    async fn health(&self) -> Result<()> {
        let body = ChatRequest {
            model: GEMINI_HEALTH_MODEL,
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
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("gemini health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("gemini health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(
            &req.user,
            &req.previous_variants_first_words,
            req.n_variants,
        );
        let model = if req.model.is_empty() {
            GEMINI_DEFAULT_MODEL
        } else {
            &req.model
        };
        let body = ChatRequest {
            model,
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
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("gemini: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("gemini http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("gemini json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("gemini: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    /// Single-shot text generation. Used by level-0 pill dispatch,
    /// rabbit-hole fan, and editor polish.
    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            GEMINI_DEFAULT_MODEL
        } else {
            &req.model
        };
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
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("gemini raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("gemini raw http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("gemini raw json: {e}")))?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("gemini raw: no choices".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn id_is_stable() {
        let p = GeminiProvider::new("test");
        assert_eq!(p.id().as_str(), "gemini");
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer gm-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "content": "[{\"angle\":\"feel\",\"text\":\"x\"},{\"angle\":\"notice\",\"text\":\"y\"},{\"angle\":\"wonder\",\"text\":\"z\"}]"
                    }
                }]
            })))
            .mount(&server)
            .await;
        let p = GeminiProvider::with_base_url("gm-secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "gemini-2.5-flash".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "z");
    }

    #[tokio::test]
    async fn generate_raw_returns_response_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "Gemini speaks."}}]
            })))
            .mount(&server)
            .await;
        let p = GeminiProvider::with_base_url("gm-secret", server.uri());
        let out = p
            .generate_raw(GenerateRequest {
                system: "s".into(),
                user: "u".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out, "Gemini speaks.");
    }

    #[tokio::test]
    async fn empty_model_falls_back_to_default() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "ok"}}]
            })))
            .mount(&server)
            .await;
        let p = GeminiProvider::with_base_url("gm-secret", server.uri());
        let _ = p
            .generate_raw(GenerateRequest {
                system: "s".into(),
                user: "u".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let received = server.received_requests().await.unwrap();
        let body = received[0].body_json::<serde_json::Value>().unwrap();
        assert_eq!(body["model"], GEMINI_DEFAULT_MODEL);
    }
}
