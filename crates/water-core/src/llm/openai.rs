use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Default model used by the single-shot `generate_raw` path when
/// `req.model` is empty. Matches the curated picker default.
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-4o-mini";

/// `LlmProvider` adapter for `OpenAI`'s Chat Completions API.
pub struct OpenAiProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl OpenAiProvider {
    /// Create a provider pointed at the public `OpenAI` API.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.openai.com")
    }

    /// Create a provider with a custom base URL (used by tests / proxies).
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
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
impl LlmProvider for OpenAiProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("openai")
    }

    async fn health(&self) -> Result<()> {
        let body = ChatRequest {
            model: "gpt-4o-mini",
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
            .map_err(|e| Error::Provider(format!("openai health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("openai health http: {e}")))?;
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
            .map_err(|e| Error::Provider(format!("openai: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("openai http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("openai json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("openai: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    /// Single-shot text generation. Used by level-0 pill dispatch,
    /// rabbit-hole fan, and editor polish — every path that calls
    /// `LlmRouter::generate_raw_with_default`.
    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            OPENAI_DEFAULT_MODEL
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
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("openai raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("openai raw http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("openai raw json: {e}")))?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("openai raw: no choices".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "[{\"angle\":\"a\",\"text\":\"1\"},{\"angle\":\"b\",\"text\":\"2\"},{\"angle\":\"c\",\"text\":\"3\"}]"}}]
            })))
            .mount(&server)
            .await;
        let p = OpenAiProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "gpt-4o-mini".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "3");
    }
}
