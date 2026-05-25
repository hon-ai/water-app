use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// `LlmProvider` adapter for Anthropic's Messages API.
pub struct AnthropicProvider {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl AnthropicProvider {
    /// Create a provider pointed at the public Anthropic API.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::with_base_url(api_key, "https://api.anthropic.com")
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
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f32,
    system: &'a str,
    messages: Vec<MessagesMessage<'a>>,
}

#[derive(Serialize)]
struct MessagesMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<MessagesContentBlock>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MessagesContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("anthropic")
    }

    async fn health(&self) -> Result<()> {
        // Anthropic has no /health endpoint; do a 1-token sanity call.
        // Claude Haiku 4.5 — cheapest current-generation model, right size
        // for a 1-token connectivity probe.
        let body = MessagesRequest {
            model: "claude-haiku-4-5-20251001",
            max_tokens: 1,
            temperature: 0.0,
            system: "Respond with the single character A and nothing else.",
            messages: vec![MessagesMessage {
                role: "user",
                content: "ping",
            }],
        };
        let r = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("anthropic health: {e}")))?;
        check_response_status(r, "anthropic health").await?;
        Ok(())
    }

    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        let user = build_user_with_exclusions(
            &req.user,
            &req.previous_variants_first_words,
            req.n_variants,
        );
        let body = MessagesRequest {
            model: &req.model,
            max_tokens: req.max_output_tokens,
            temperature: req.temperature,
            system: &req.system,
            messages: vec![MessagesMessage {
                role: "user",
                content: &user,
            }],
        };
        let r = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("anthropic: {e}")))?;
        let r = check_response_status(r, "anthropic").await?;
        let resp: MessagesResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("anthropic json: {e}")))?;
        let text = resp
            .content
            .into_iter()
            .map(|b| match b {
                MessagesContentBlock::Text { text } => text,
            })
            .next()
            .ok_or_else(|| Error::Provider("anthropic: no text block".into()))?;
        parse_bouquet_json(&text, req.n_variants)
    }

    async fn generate_raw(&self, req: GenerateRequest) -> Result<String> {
        // Single-shot text generation. Used by orchestrator-side Stage 2
        // confirmation prompts and by the level-0 pill text generation
        // path. When the caller leaves `req.model` empty (the
        // `GenerateRequest::default()` shape used by
        // `LlmRouter::generate_raw_with_default`), fall back to Sonnet
        // 4.6: Haiku showed a strong "no" bias on Stage 2 confirmation
        // calls (it correctly noticed that subtly-worded contradictions
        // could coexist) and the level-0 path benefits from Sonnet's
        // observational quality. Callers that need a different model
        // (e.g. health checks using Haiku) supply it explicitly.
        let model = if req.model.is_empty() {
            "claude-sonnet-4-6"
        } else {
            &req.model
        };
        let max_tokens = if req.max_output_tokens == 0 {
            // Sized for the M3 Stage 2 confirmation prompt (1 token of
            // "yes"/"no" plus a small buffer for whitespace) plus the M2
            // level-0 budget when the level-0 path forgets to set it.
            512
        } else {
            req.max_output_tokens
        };
        let body = MessagesRequest {
            model,
            max_tokens,
            temperature: req.temperature,
            system: &req.system,
            messages: vec![MessagesMessage {
                role: "user",
                content: &req.user,
            }],
        };
        let r = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Provider(format!("anthropic: {e}")))?;
        let r = check_response_status(r, "anthropic").await?;
        let resp: MessagesResponse = r
            .json()
            .await
            .map_err(|e| Error::Provider(format!("anthropic json: {e}")))?;
        resp.content
            .into_iter()
            .map(|b| match b {
                MessagesContentBlock::Text { text } => text,
            })
            .next()
            .ok_or_else(|| Error::Provider("anthropic: no text block".into()))
    }
}

/// Convert a non-2xx response into a `Provider` error that includes the
/// server's response body. `reqwest::Response::error_for_status` discards
/// the body, so the caller would never see Anthropic / OpenAI's structured
/// error JSON (`{"error":{"type":"invalid_request_error","message":"..."}}`).
/// Without the body, a 400 is unactionable; with it, the writer sees the
/// actual reason.
async fn check_response_status(r: reqwest::Response, provider: &str) -> Result<reqwest::Response> {
    let status = r.status();
    if status.is_success() {
        return Ok(r);
    }
    let body = r
        .text()
        .await
        .unwrap_or_else(|_| "(failed to read response body)".into());
    Err(Error::Provider(format!(
        "{provider} http {status}: {body}"
    )))
}

pub(super) fn build_user_with_exclusions(base: &str, prior: &[String], n: usize) -> String {
    let mut s = String::with_capacity(base.len() + 128);
    s.push_str(base);
    s.push_str("\n\nReturn exactly ");
    s.push_str(&n.to_string());
    s.push_str(" items as a strict JSON array: [{\"angle\":\"...\",\"text\":\"...\"}].");
    if !prior.is_empty() {
        s.push_str(" Previous openings to avoid: ");
        for (i, p) in prior.iter().enumerate() {
            if i > 0 {
                s.push_str("; ");
            }
            s.push('"');
            s.push_str(p);
            s.push('"');
        }
        s.push('.');
    }
    s
}

pub(super) fn parse_bouquet_json(text: &str, n: usize) -> Result<Vec<BouquetVariant>> {
    let trimmed = text.trim();
    let start = trimmed
        .find('[')
        .ok_or_else(|| Error::Provider("no JSON array".into()))?;
    let end = trimmed
        .rfind(']')
        .ok_or_else(|| Error::Provider("no JSON array close".into()))?;
    if end <= start {
        return Err(Error::Provider("malformed JSON array".into()));
    }
    let json = &trimmed[start..=end];
    let parsed: Vec<BouquetVariant> =
        serde_json::from_str(json).map_err(|e| Error::Provider(format!("bouquet json: {e}")))?;
    if parsed.len() < n {
        return Err(Error::Provider(format!(
            "bouquet had {} items, expected {}",
            parsed.len(),
            n
        )));
    }
    Ok(parsed.into_iter().take(n).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn generate_bouquet_parses_three_variants() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "content": [{
                "type": "text",
                "text": "[{\"angle\":\"feel\",\"text\":\"a\"},{\"angle\":\"notice\",\"text\":\"b\"},{\"angle\":\"wonder\",\"text\":\"c\"}]"
            }]
        });
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "claude-3-5-sonnet-latest".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].angle, "notice");
    }

    #[tokio::test]
    async fn generate_bouquet_errors_when_too_few_variants() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type":"text","text":"[{\"angle\":\"feel\",\"text\":\"only one\"}]"}]
            })))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        let req = BouquetRequest {
            system: "tone".into(),
            user: "react".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "m".into(),
            temperature: 0.7,
            max_output_tokens: 200,
        };
        assert!(p.generate_bouquet(&req).await.is_err());
    }

    #[tokio::test]
    async fn health_passes_when_api_returns_200() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type":"text","text":"A"}]
            })))
            .mount(&server)
            .await;
        let p = AnthropicProvider::with_base_url("secret", server.uri());
        assert!(p.health().await.is_ok());
    }
}
