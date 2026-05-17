//! `MLX` adapter — Apple silicon. v1 stub; real impl in v1.x once benchmarked.

use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use async_trait::async_trait;

pub struct MlxProvider {
    pub model_path: String,
}

impl MlxProvider {
    #[must_use]
    pub fn new(model_path: impl Into<String>) -> Self {
        Self {
            model_path: model_path.into(),
        }
    }
}

#[async_trait]
impl LlmProvider for MlxProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("mlx")
    }

    async fn health(&self) -> Result<()> {
        Err(Error::Provider(
            "MLX adapter is a v1 stub; enable feature `mlx` and provide a real implementation"
                .into(),
        ))
    }

    async fn generate_bouquet(&self, _req: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
        Err(Error::Provider("MLX adapter is a v1 stub".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_health_returns_error() {
        let p = MlxProvider::new("dummy.mlx");
        assert!(p.health().await.is_err());
    }
}
