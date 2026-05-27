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
    pub api_key_encrypted: Vec<u8>,
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
    pub fn decrypt_api_key(
        &self,
        encryptor: &crate::encryption::Encryptor,
    ) -> Result<String, crate::encryption::EncryptionError> {
        let key_bytes = encryptor.decrypt(&self.api_key_encrypted)?;
        String::from_utf8(key_bytes)
            .map_err(|_| crate::encryption::EncryptionError::DecryptionFailed)
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
            api_key_encrypted: vec![115, 117, 112, 101, 114, 45, 115, 101, 99, 114, 101, 116],
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
