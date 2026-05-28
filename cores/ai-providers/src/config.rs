//! Configuration structures for AI providers.
//!
//! [`ProviderConfig`] stores all provider metadata needed to call an LLM API.
//! The `api_key_encrypted` field is intentionally redacted in `Debug` output.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Metadata for one model exposed by a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
}

/// Configuration for an AI provider.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: i64,
    pub name: String,
    pub base_url: String,
    pub api_key_encrypted: Option<Vec<u8>>,
    #[serde(default = "default_auth_header_name")]
    pub auth_header_name: String,
    #[serde(default = "default_auth_header_value_prefix")]
    pub auth_header_value_prefix: String,
    pub models: Vec<ModelInfo>,
    pub request_body_template: serde_json::Value,
    pub response_path: String,
}

impl ProviderConfig {
    /// Decrypt the provider's encrypted API key for in-memory use by callers.
    ///
    /// Returns `Ok(None)` when the provider has no API key configured (seed profile).
    pub fn decrypt_api_key(
        &self,
        encryptor: &crate::encryption::Encryptor,
    ) -> Result<Option<String>, crate::encryption::EncryptionError> {
        match &self.api_key_encrypted {
            Some(encrypted) if !encrypted.is_empty() => {
                let key_bytes = encryptor.decrypt(encrypted)?;
                String::from_utf8(key_bytes)
                    .map(Some)
                    .map_err(|_| crate::encryption::EncryptionError::DecryptionFailed)
            }
            _ => Ok(None),
        }
    }

    /// Returns `true` if this provider has an API key configured.
    pub fn has_api_key(&self) -> bool {
        self.api_key_encrypted
            .as_ref()
            .is_some_and(|v| !v.is_empty())
    }
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key_encrypted", &"[REDACTED]")
            .field("auth_header_name", &self.auth_header_name)
            .field("auth_header_value_prefix", &self.auth_header_value_prefix)
            .field("models", &self.models)
            .field("request_body_template", &self.request_body_template)
            .field("response_path", &self.response_path)
            .finish()
    }
}

pub fn default_auth_header_name() -> String {
    "Authorization".to_string()
}

pub fn default_auth_header_value_prefix() -> String {
    "Bearer ".to_string()
}

/// Pre-configured (key-less) provider profiles for common LLM services.
///
/// Each profile has all fields populated except `api_key_encrypted`, which is
/// left empty so the user supplies their own key.
pub mod default_profiles {
    use super::ModelInfo;
    use crate::storage::NewProviderConfig;

    /// OpenAI provider profile (key-less seed).
    pub fn openai() -> NewProviderConfig {
        NewProviderConfig {
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: None,
            auth_header_name: None,         // uses default "Authorization"
            auth_header_value_prefix: None, // uses default "Bearer "
            models: vec![
                ModelInfo {
                    id: "gpt-4o".to_string(),
                    name: "GPT-4o".to_string(),
                    context_window: 128_000,
                },
                ModelInfo {
                    id: "gpt-4o-mini".to_string(),
                    name: "GPT-4o mini".to_string(),
                    context_window: 128_000,
                },
            ],
            request_body_template: serde_json::json!({
                "model": "{model}",
                "messages": "{messages}",
                "stream": "{stream}",
                "temperature": "{temperature}"
            }),
            response_path: "choices[0].message.content".to_string(),
        }
    }

    /// Anthropic Claude provider profile (key-less seed).
    ///
    /// The adapter adds Anthropic's required `anthropic-version: 2023-06-01`
    /// request header automatically for `api.anthropic.com` URLs.
    pub fn anthropic() -> NewProviderConfig {
        NewProviderConfig {
            name: "Anthropic".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
            api_key: None,
            auth_header_name: Some("x-api-key".to_string()),
            auth_header_value_prefix: Some(String::new()),
            models: vec![
                ModelInfo {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    context_window: 200_000,
                },
                ModelInfo {
                    id: "claude-3-5-haiku-20241022".to_string(),
                    name: "Claude 3.5 Haiku".to_string(),
                    context_window: 200_000,
                },
            ],
            // Anthropic Messages API requires max_tokens. Keep stream and temperature
            // configurable through the generic adapter template.
            request_body_template: serde_json::json!({
                "model": "{model}",
                "messages": "{messages}",
                "max_tokens": 1024,
                "stream": "{stream}",
                "temperature": "{temperature}"
            }),
            response_path: "content[0].text".to_string(),
        }
    }

    /// OpenRouter provider profile (key-less seed).
    ///
    /// OpenRouter is OpenAI-compatible, so this profile uses the same template
    /// and auth scheme as OpenAI.
    pub fn openrouter() -> NewProviderConfig {
        NewProviderConfig {
            name: "OpenRouter".to_string(),
            base_url: "https://openrouter.ai/api/v1/chat/completions".to_string(),
            api_key: None,
            auth_header_name: None,
            auth_header_value_prefix: None,
            models: vec![
                ModelInfo {
                    id: "openai/gpt-4o".to_string(),
                    name: "GPT-4o (via OpenRouter)".to_string(),
                    context_window: 128_000,
                },
                ModelInfo {
                    id: "anthropic/claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4 (via OpenRouter)".to_string(),
                    context_window: 200_000,
                },
                ModelInfo {
                    id: "google/gemini-2.5-flash".to_string(),
                    name: "Gemini 2.5 Flash (via OpenRouter)".to_string(),
                    context_window: 1_048_576,
                },
            ],
            request_body_template: serde_json::json!({
                "model": "{model}",
                "messages": "{messages}",
                "stream": "{stream}",
                "temperature": "{temperature}"
            }),
            response_path: "choices[0].message.content".to_string(),
        }
    }

    /// All default provider profiles.
    pub fn all() -> Vec<NewProviderConfig> {
        vec![openai(), anthropic(), openrouter()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_config_deserializes_valid_json_with_defaults() {
        let json = r#"
        {
          "id": 1,
          "name": "OpenAI",
          "base_url": "https://api.openai.com/v1",
          "api_key_encrypted": [1, 2, 3],
          "models": [
            {"id": "gpt-4o", "name": "GPT-4o", "context_window": 128000}
          ],
          "request_body_template": {
            "model": "{model}",
            "messages": "{messages}",
            "stream": "{stream}",
            "temperature": "{temperature}"
          },
          "response_path": "choices[0].message.content"
        }
        "#;

        let config: ProviderConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.id, 1);
        assert_eq!(config.auth_header_name, "Authorization");
        assert_eq!(config.auth_header_value_prefix, "Bearer ");
        assert_eq!(config.models[0].id, "gpt-4o");
        assert_eq!(config.models[0].context_window, 128000);
        assert!(config.has_api_key());
    }

    #[test]
    fn provider_config_deserializes_null_api_key() {
        let json = r#"
        {
          "id": 2,
          "name": "OpenAI Seed",
          "base_url": "https://api.openai.com/v1/chat/completions",
          "api_key_encrypted": null,
          "models": [
            {"id": "gpt-4o", "name": "GPT-4o", "context_window": 128000}
          ],
          "request_body_template": {"model": "{model}", "messages": "{messages}"},
          "response_path": "choices[0].message.content"
        }
        "#;

        let config: ProviderConfig = serde_json::from_str(json).unwrap();
        assert!(!config.has_api_key());
        assert_eq!(config.api_key_encrypted, None);
    }

    #[test]
    fn provider_config_deserializes_valid_json_with_custom_auth_fields() {
        let json = r#"
        {
          "id": 7,
          "name": "Custom",
          "base_url": "https://api.example.com",
          "api_key_encrypted": [9, 8, 7],
          "auth_header_name": "X-API-Key",
          "auth_header_value_prefix": "",
          "models": [],
          "request_body_template": {"model": "{model}"},
          "response_path": "content"
        }
        "#;

        let config: ProviderConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.auth_header_name, "X-API-Key");
        assert_eq!(config.auth_header_value_prefix, "");
    }

    #[test]
    fn provider_config_rejects_invalid_json() {
        let json = r#"
        {
          "id": 1,
          "name": "Broken",
          "base_url": "https://api.example.com",
          "api_key_encrypted": [1, 2, 3],
          "models": [
            {"id": "missing-context-window", "name": "Broken"}
          ],
          "request_body_template": {},
          "response_path": "content"
        }
        "#;

        let error = serde_json::from_str::<ProviderConfig>(json).unwrap_err();
        assert!(error.to_string().contains("context_window"));
    }

    #[test]
    fn debug_output_redacts_encrypted_api_key() {
        let config = ProviderConfig {
            id: 1,
            name: "test-provider".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key_encrypted: Some(vec![
                115, 117, 112, 101, 114, 45, 115, 101, 99, 114, 101, 116,
            ]),
            auth_header_name: default_auth_header_name(),
            auth_header_value_prefix: default_auth_header_value_prefix(),
            models: vec![ModelInfo {
                id: "gpt-4".to_string(),
                name: "GPT-4".to_string(),
                context_window: 8192,
            }],
            request_body_template: serde_json::json!({"model": "{model}"}),
            response_path: "choices[0].message.content".to_string(),
        };

        let debug_output = format!("{config:?}");

        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super-secret"));
        assert!(!debug_output.contains("115"));
        assert!(debug_output.contains("test-provider"));
        assert!(debug_output.contains("gpt-4"));
    }
}
