//! Configuration structures for AI providers.

use std::fmt;

/// Configuration for an AI provider.
#[derive(Clone)]
pub struct ProviderConfig {
    /// The name of the provider (e.g., "openai", "anthropic").
    pub name: String,
    /// The base URL for the provider's API.
    pub base_url: String,
    /// The API key for authentication (redacted in Debug output).
    pub api_key: String,
    /// The model identifier to use.
    pub model: String,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .finish()
    }
}
