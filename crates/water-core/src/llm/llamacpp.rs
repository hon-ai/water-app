use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// `LlmProvider` adapter for a local `llama.cpp` server running in
/// `OpenAI`-compatible mode (`/v1/chat/completions`). Auth is optional.
pub struct LlamaCppProvider {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl LlamaCppProvider {
    /// Create a provider pointed at a `llama.cpp` base URL, without auth.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: None,
            http: reqwest::Client::builder()
                .timeout(Duration::from_mins(1))
                .build()
                .expect("reqwest"),
        }
    }

    /// Create a provider pointed at a `llama.cpp` base URL with an API key
    /// sent as a `Bearer` token in the `Authorization` header.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn with_api_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: Some(key.into()),
            http: reqwest::Client::builder()
                .timeout(Duration::from_mins(1))
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
impl LlmProvider for LlamaCppProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("llamacpp")
    }

    async fn health(&self) -> Result<()> {
        let r = self
            .http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Provider(format!("llamacpp health: {e}")))?;
        r.error_for_status()
            .map_err(|e| Error::Provider(format!("llamacpp health http: {e}")))?;
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
        let mut req_builder = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body);
        if let Some(k) = &self.api_key {
            req_builder = req_builder.bearer_auth(k);
        }
        let r = req_builder
            .send()
            .await
            .map_err(|e| Error::Provider(format!("llamacpp: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("llamacpp http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("llamacpp json: {e}")))?;
        let text = resp
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("llamacpp: no choices".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    /// Single-shot text generation. Used by level-0 pill dispatch.
    /// `req.model` defaults to "default" — llama.cpp's server picks
    /// whatever model it was loaded with.
    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            "default"
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
        let mut req_builder = self
            .http
            .post(format!("{}/v1/chat/completions", self.base_url))
            .json(&body);
        if let Some(k) = &self.api_key {
            req_builder = req_builder.bearer_auth(k);
        }
        let r = req_builder
            .send()
            .await
            .map_err(|e| Error::Provider(format!("llamacpp raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("llamacpp raw http: {e}")))?;
        let resp: ChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("llamacpp raw json: {e}")))?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Provider("llamacpp raw: no choices".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn health_passes_on_200_health() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok"})),
            )
            .mount(&server)
            .await;
        let p = LlamaCppProvider::new(server.uri());
        assert!(p.health().await.is_ok());
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices":[{"message":{"content":"[{\"angle\":\"a\",\"text\":\"1\"},{\"angle\":\"b\",\"text\":\"2\"},{\"angle\":\"c\",\"text\":\"3\"}]"}}]
            })))
            .mount(&server)
            .await;
        let p = LlamaCppProvider::new(server.uri());
        let req = BouquetRequest {
            system: "s".into(),
            user: "u".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "kimi-k2-q4".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "1");
    }
}
