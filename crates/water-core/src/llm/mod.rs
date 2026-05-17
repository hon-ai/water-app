//! LLM provider trait + concrete adapters + router.

pub mod anthropic;
pub mod llamacpp;
pub mod ollama;
pub mod openai;
pub mod provider;
pub use anthropic::AnthropicProvider;
pub use llamacpp::LlamaCppProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use provider::*;
