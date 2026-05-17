//! LLM provider trait + concrete adapters + router.

pub mod anthropic;
pub mod openai;
pub mod provider;
pub use anthropic::AnthropicProvider;
pub use openai::OpenAiProvider;
pub use provider::*;
