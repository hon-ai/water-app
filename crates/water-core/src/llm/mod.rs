//! LLM provider trait + concrete adapters + router.

pub mod anthropic;
pub mod llamacpp;
pub mod ollama;
pub mod openai;
pub mod provider;
pub mod router;
pub mod secrets;
pub use anthropic::AnthropicProvider;
pub use llamacpp::LlamaCppProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use provider::*;
pub use router::{LlmRouter, ProviderHealthChange, RateLimitConfig};
pub use secrets::Secrets;

#[cfg(feature = "mlx")]
pub mod mlx;
#[cfg(feature = "mlx")]
pub use mlx::MlxProvider;
