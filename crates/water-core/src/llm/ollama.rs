use super::anthropic::{build_user_with_exclusions, parse_bouquet_json};
use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};

/// Default model when `req.model` is empty. Matches the curated
/// picker default — small, broadly available on a fresh Ollama install.
pub const OLLAMA_DEFAULT_MODEL: &str = "qwen2.5:3b";
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// `LlmProvider` adapter for a local `Ollama` server (`/api/chat`).
pub struct OllamaProvider {
    base_url: String,
    http: reqwest::Client,
}

impl OllamaProvider {
    /// Create a provider pointed at a custom `Ollama` base URL.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_mins(1))
                .build()
                .expect("reqwest"),
        }
    }

    /// Create a provider pointed at the default local `Ollama` URL
    /// (`http://127.0.0.1:11434`).
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn default_url() -> Self {
        Self::new("http://127.0.0.1:11434")
    }
}

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    stream: bool,
    options: OllamaOptions,
    messages: Vec<OllamaMessage<'a>>,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageOut,
}

#[derive(Deserialize)]
struct OllamaMessageOut {
    content: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    #[allow(dead_code)]
    models: Vec<serde_json::Value>,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("ollama")
    }

    async fn health(&self) -> Result<()> {
        let r = self
            .http
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| Error::Provider(format!("ollama health: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("ollama health http: {e}")))?;
        let _tags: OllamaTagsResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("ollama tags json: {e}")))?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(
            &req.user,
            &req.previous_variants_first_words,
            req.n_variants,
        );
        let body = OllamaChatRequest {
            model: &req.model,
            stream: false,
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict: req.max_output_tokens,
            },
            messages: vec![
                OllamaMessage {
                    role: "system",
                    content: &req.system,
                },
                OllamaMessage {
                    role: "user",
                    content: &user,
                },
            ],
        };
        let r = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("ollama: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("ollama http: {e}")))?;
        let resp: OllamaChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("ollama json: {e}")))?;
        parse_bouquet_json(&resp.message.content, req.n_variants)
    }

    /// Single-shot text generation. Used by level-0 pill dispatch.
    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        let model = if req.model.is_empty() {
            OLLAMA_DEFAULT_MODEL
        } else {
            &req.model
        };
        // num_predict=0 on Ollama means "generate nothing"; clamp.
        let num_predict = if req.max_output_tokens == 0 {
            512
        } else {
            req.max_output_tokens
        };
        let body = OllamaChatRequest {
            model,
            stream: false,
            options: OllamaOptions {
                temperature: req.temperature,
                num_predict,
            },
            messages: vec![
                OllamaMessage {
                    role: "system",
                    content: &req.system,
                },
                OllamaMessage {
                    role: "user",
                    content: &req.user,
                },
            ],
        };
        let r = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("ollama raw: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Provider(format!("ollama raw http: {e}")))?;
        let resp: OllamaChatResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("ollama raw json: {e}")))?;
        Ok(resp.message.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn health_succeeds_when_tags_returns_ok() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"models":[]})),
            )
            .mount(&server)
            .await;
        let p = OllamaProvider::new(server.uri());
        assert!(p.health().await.is_ok());
    }

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "message":{"content":"[{\"angle\":\"a\",\"text\":\"x\"},{\"angle\":\"b\",\"text\":\"y\"},{\"angle\":\"c\",\"text\":\"z\"}]"}
            })))
            .mount(&server)
            .await;
        let p = OllamaProvider::new(server.uri());
        let req = BouquetRequest {
            system: "s".into(),
            user: "u".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "qwen2.5:3b".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].text, "y");
    }
}
