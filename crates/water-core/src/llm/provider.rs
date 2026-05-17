use crate::Result;
use async_trait::async_trait;
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
}

/// A canned provider used by tests and by the M1 `provider.test` command
/// when no real provider is configured.
pub struct CannedProvider;

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn canned_provider_returns_requested_count() {
        let p = CannedProvider;
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
        assert!(CannedProvider.health().await.is_ok());
    }
}
