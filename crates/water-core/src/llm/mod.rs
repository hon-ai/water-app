//! LLM provider trait + concrete adapters + router.

pub mod anthropic;
pub mod provider;
pub use anthropic::AnthropicProvider;
pub use provider::*;
