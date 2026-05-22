//! OpenRouter (`openrouter.ai`) `LlmProvider` adapter.
//!
//! OpenRouter exposes an OpenAI-compatible Chat Completions API at
//! `https://openrouter.ai/api/v1`. The wire format matches OpenAI's
//! exactly, so the request / response shape mirrors `super::openai`.
//! Distinct adapter rather than "point OpenAI at a different
//! base URL" so:
//!
//!   1. A stable `ProviderId("openrouter")` is available for
//!      routing, per-provider cost tracking, secrets keying, and
//!      the renderer's Settings UI grouping.
//!   2. The default model is one of the long-context options
//!      OpenRouter aggregates (Kimi K2 by default — same reason
//!      Water carries a dedicated Kimi adapter). Writers can swap
//!      to any other model on the OpenRouter catalog via the
//!      Settings model picker.
//!   3. The health-check model is the cheapest viable OpenRouter
//!      entry so connectivity probes don't burn flagship-model
//!      quota.
//!
//! **Security**: the API key is never embedded in source. Writers
//! save it via the existing `provider_set_key("openrouter", ...)`
//! flow, which persists to the secrets store the rest of the
//! provider adapters use. No key handling lives in this file.

use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const OPENROUTER_DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Default model for new OpenRouter installations. Kimi K2 has the
/// largest context window on the catalog (256k) — the *reason* a
/// writer would route through OpenRouter is long-context work.
pub const OPENROUTER_DEFAULT_MODEL: &str = "moonshotai/kimi-k2";

/// Cheap free-tier model used by `health()` probes so connectivity
/// checks don't spend the budget OpenRouter caps free accounts at.
pub const OPENROUTER_HEALTH_MODEL: &str = "meta-llama/llama-3.2-3b-instruct:free";

pub struct OpenRouterProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl OpenRouterProvider {
    /// Create a provider pointed at the public OpenRouter endpoint.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, OPENROUTER_DEFAULT_BASE_URL)
    }

    /// Create with a custom base URL. Test stubs (wiremock) take
    /// this path; production always uses `new`.
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
impl LlmProvider for OpenRouterProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("openrouter")
    }

    async fn health(&self) -> Result<()> {
        let body = ChatRequest {
            model: OPENROUTER_HEALTH_MODEL,
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
            // OpenRouter recommends these headers so usage shows the
            // referring app in their dashboard. Optional but polite.
            .header("HTTP-Referer", "https://water-app.local")
            .header("X-Title", "Water")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openrouter health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("openrouter health http: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(
            &req.user,
            &req.previous_variants_first_words,
            req.n_variants,
        );
        let model = if req.model.is_empty() {
            OPENROUTER_DEFAULT_MODEL
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
            .header("HTTP-Referer", "https://water-app.local")
            .header("X-Title", "Water")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openrouter: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("openrouter http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("openrouter json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("openrouter: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            OPENROUTER_DEFAULT_MODEL
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
            .header("HTTP-Referer", "https://water-app.local")
            .header("X-Title", "Water")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openrouter raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("openrouter raw http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("openrouter raw json: {e}")))?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("openrouter raw: no choices".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn id_is_stable() {
        let p = OpenRouterProvider::new("test");
        assert_eq!(p.id().as_str(), "openrouter");
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .and(header("authorization", "Bearer or-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {
                        "content": "[{\"angle\":\"feel\",\"text\":\"a\"},{\"angle\":\"notice\",\"text\":\"b\"},{\"angle\":\"wonder\",\"text\":\"c\"}]"
                    }
                }]
            })))
            .mount(&server)
            .await;
        let p = OpenRouterProvider::with_base_url("or-secret", server.uri());
        let req = BouquetRequest {
            system: "sys".into(),
            user: "u".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "moonshotai/kimi-k2".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "c");
    }

    #[tokio::test]
    async fn generate_raw_returns_response_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "hello"}}]
            })))
            .mount(&server)
            .await;
        let p = OpenRouterProvider::with_base_url("or-secret", server.uri());
        let out = p
            .generate_raw(GenerateRequest {
                system: "s".into(),
                user: "u".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(out, "hello");
    }

    #[tokio::test]
    async fn empty_model_falls_back_to_default_kimi() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "ok"}}]
            })))
            .mount(&server)
            .await;
        let p = OpenRouterProvider::with_base_url("or-secret", server.uri());
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
        assert_eq!(body["model"], OPENROUTER_DEFAULT_MODEL);
    }
}
