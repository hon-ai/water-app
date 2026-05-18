use crate::{Error, Result};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Identifier for a configured provider instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(pub String);

impl ProviderId {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BouquetRequest {
    pub system: String,
    pub user: String,
    pub n_variants: usize,
    /// First 8 words of each previously-generated variant in this rabbit
    /// hole, to push the model toward novelty when regenerating.
    #[serde(default)]
    pub previous_variants_first_words: Vec<String>,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BouquetVariant {
    pub angle: String,
    pub text: String,
}

/// Generic single-shot generation request used by `generate_raw` /
/// `generate_structured`. Distinct from `BouquetRequest`, which is the
/// specialized M1 path that returns parsed `Vec<BouquetVariant>` per
/// adapter's native JSON handling.
///
/// M2 introduces this generic path so post-filtered tasks (regenerate, expand)
/// and other prompts can request raw text — or, via `generate_structured`,
/// any `DeserializeOwned` type — without forcing every task through the
/// bouquet-shape contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub system: String,
    pub user: String,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Stable identifier (e.g. `"anthropic"`, `"ollama"`, `"llamacpp-kimi"`).
    fn id(&self) -> ProviderId;

    /// Cheap connectivity check. Implementations may issue a 1-token call,
    /// hit a `/health` endpoint, or just validate that credentials exist.
    async fn health(&self) -> Result<()>;

    /// Generate exactly `req.n_variants` bouquet items. Adapters must
    /// validate the model returned exactly that many; if it returned more,
    /// truncate; if fewer, error.
    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>>;

    /// Generic single-shot text generation. Returns the raw model output.
    ///
    /// The default implementation returns `Error::Provider` indicating the
    /// adapter has not opted in. Real adapters (OpenAI/Anthropic/Ollama/
    /// llamacpp/MLX) override this when M2 wires structured-JSON tasks
    /// through them. `CannedProvider` overrides this to support unit tests.
    async fn generate_raw(&self, _req: GenerateRequest) -> Result<String> {
        Err(Error::Provider(format!(
            "generate_raw not implemented for provider {}",
            self.id().as_str()
        )))
    }

    /// Provider-specific structured JSON path. The default implementation
    /// calls `generate_raw` and parses the resulting text as JSON. Per-
    /// provider overrides can use native JSON-schema or grammar-constrained
    /// modes for higher reliability (`OpenAI` `response_format`, Anthropic
    /// tool-use, `llamacpp` grammars). M2 ships the default only; native
    /// overrides land as follow-ups if integration testing surfaces drops.
    ///
    /// `where Self: Sized` keeps the trait dyn-compatible: callers that hold
    /// `Arc<dyn LlmProvider>` use `generate_raw` directly and deserialize
    /// themselves; callers that hold a concrete provider get this convenience
    /// wrapper.
    async fn generate_structured<T: DeserializeOwned + Send>(
        &self,
        req: GenerateRequest,
    ) -> Result<T>
    where
        Self: Sized,
    {
        let raw = self.generate_raw(req).await?;
        serde_json::from_str::<T>(&raw)
            .map_err(|e| Error::Provider(format!("invalid json: {e}; raw: {raw}")))
    }
}

/// A canned provider used by tests and by the M1 `provider.test` command
/// when no real provider is configured.
///
/// By default, `generate_bouquet` returns synthetic variants and
/// `generate_raw` returns an empty string. Use [`CannedProvider::with_response`]
/// to pre-load a fixed raw response (drives the M2 structured-JSON tests).
#[derive(Default)]
pub struct CannedProvider {
    raw_response: Option<String>,
}

impl CannedProvider {
    /// Pre-load a fixed raw response that `generate_raw` (and therefore
    /// `generate_structured`) will return. Used by tests.
    #[must_use]
    pub fn with_response(raw: impl Into<String>) -> Self {
        Self {
            raw_response: Some(raw.into()),
        }
    }
}

#[async_trait]
impl LlmProvider for CannedProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("canned")
    }
    async fn health(&self) -> Result<()> {
        Ok(())
    }
    async fn generate_bouquet(&self, req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        Ok((0..req.n_variants)
            .map(|i| BouquetVariant {
                angle: ["feel", "notice", "wonder"][i % 3].into(),
                text: format!("(canned variant {} of {})", i + 1, req.n_variants),
            })
            .collect())
    }
    async fn generate_raw(&self, _req: GenerateRequest) -> Result<String> {
        Ok(self.raw_response.clone().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn canned_provider_returns_requested_count() {
        let p = CannedProvider::default();
        let req = BouquetRequest {
            system: "tone".into(),
            user: "Hello".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "canned".into(),
            temperature: 0.7,
            max_output_tokens: 80,
        };
        let out = p.generate_bouquet(&req).await.unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].angle, "feel");
    }

    #[tokio::test]
    async fn canned_provider_health_ok() {
        assert!(CannedProvider::default().health().await.is_ok());
    }
}

#[cfg(test)]
mod structured_tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct BouquetItem {
        angle: String,
        #[allow(dead_code)]
        text: String,
    }

    #[tokio::test]
    async fn default_structured_parses_json_via_canned() {
        let canned = CannedProvider::with_response(
            r#"[{"angle":"feel","text":"a"},{"angle":"notice","text":"b"},{"angle":"wonder","text":"c"}]"#,
        );
        let req = GenerateRequest::default();
        let bouquet: Vec<BouquetItem> = canned.generate_structured(req).await.unwrap();
        assert_eq!(bouquet.len(), 3);
        assert_eq!(bouquet[0].angle, "feel");
    }

    #[tokio::test]
    async fn default_structured_errors_on_invalid_json() {
        let canned = CannedProvider::with_response("not json at all");
        let req = GenerateRequest::default();
        let result: Result<Vec<BouquetItem>> = canned.generate_structured(req).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn default_generate_raw_errors_on_unimplemented_provider() {
        // Custom provider that does NOT override generate_raw — should hit
        // the default impl and return Error::Provider.
        struct NoRaw;
        #[async_trait]
        impl LlmProvider for NoRaw {
            fn id(&self) -> ProviderId {
                ProviderId::new("no-raw")
            }
            async fn health(&self) -> Result<()> {
                Ok(())
            }
            async fn generate_bouquet(&self, _: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
                Ok(vec![])
            }
        }
        let p = NoRaw;
        let err = p
            .generate_raw(GenerateRequest::default())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Provider(_)));
    }
}
