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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_output_redacts_api_key() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "super-secret-api-key-12345".to_string(),
            model: "gpt-4".to_string(),
        };

        let debug_output = format!("{:?}", config);

        // Should contain [REDACTED]
        assert!(debug_output.contains("[REDACTED]"));

        // Should NOT contain the actual API key
        assert!(!debug_output.contains("super-secret-api-key-12345"));

        // Should contain other fields
        assert!(debug_output.contains("test-provider"));
        assert!(debug_output.contains("https://api.example.com"));
        assert!(debug_output.contains("gpt-4"));
    }

    #[test]
    fn test_debug_output_with_empty_api_key() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "".to_string(),
            model: "gpt-4".to_string(),
        };

        let debug_output = format!("{:?}", config);

        // Should still show [REDACTED] even for empty key
        assert!(debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn test_clone_preserves_data() {
        let config = ProviderConfig {
            name: "test-provider".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: "secret-key".to_string(),
            model: "gpt-4".to_string(),
        };

        let cloned = config.clone();

        assert_eq!(config.name, cloned.name);
        assert_eq!(config.base_url, cloned.base_url);
        assert_eq!(config.api_key, cloned.api_key);
        assert_eq!(config.model, cloned.model);
    }
}
