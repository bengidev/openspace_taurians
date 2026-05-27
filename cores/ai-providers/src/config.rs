//! Configuration structures for AI providers.
//!
//! [`ProviderConfig`] supports encrypting its `api_key` at rest via the
//! [`crate::encryption::Encryptor`] API. The `api_key` field is redacted in
//! `Debug` output.

use crate::encryption::Encryptor;
use std::fmt;

/// Configuration for an AI provider.
///
/// The `api_key` field is stored in memory as plaintext but can be encrypted
/// for persistence via [`ProviderConfig::encrypt_api_key`] and decrypted via
/// [`ProviderConfig::decrypt_api_key`].
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

impl ProviderConfig {
    /// Encrypt the `api_key` field using the given encryptor.
    ///
    /// Returns the encrypted key (nonce + ciphertext) suitable for storage.
    /// The plaintext `api_key` remains in memory.
    pub fn encrypt_api_key(&self, encryptor: &Encryptor) -> Result<Vec<u8>, crate::encryption::EncryptionError> {
        encryptor.encrypt(self.api_key.as_bytes())
    }

    /// Replace the `api_key` field by decrypting `encrypted_key` with the given encryptor.
    ///
    /// The decrypted key is stored in `self.api_key` as a plaintext [`String`].
    pub fn decrypt_api_key(&mut self, encryptor: &Encryptor, encrypted_key: &[u8]) -> Result<(), crate::encryption::EncryptionError> {
        let key_bytes = encryptor.decrypt(encrypted_key)?;
        self.api_key = String::from_utf8(key_bytes).map_err(|_| crate::encryption::EncryptionError::DecryptionFailed)?;
        Ok(())
    }
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
    use crate::encryption::Encryptor;

    struct TestDir {
        path: std::path::PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let dir = std::env::temp_dir().join(format!(
                "config_test_{}_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                rng.gen::<u64>()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self { path: dir }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

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
    fn test_encrypt_decrypt_api_key_round_trip() {
        let test_dir = TestDir::new();
        let encryptor = Encryptor::new(test_dir.path()).unwrap();

        let mut config = ProviderConfig {
            name: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            api_key: "sk-this-is-a-secret-key".to_string(),
            model: "gpt-4".to_string(),
        };

        // Encrypt the key
        let encrypted = config.encrypt_api_key(&encryptor).unwrap();

        // The encrypted data should be longer than the plaintext (nonce + ciphertext + tag)
        assert!(encrypted.len() > 12);

        // Replace key with garbage, then restore via decryption
        config.api_key = "garbage".to_string();
        config.decrypt_api_key(&encryptor, &encrypted).unwrap();
        assert_eq!(config.api_key, "sk-this-is-a-secret-key");
    }

    #[test]
    fn test_encrypt_api_key_different_encryptors_produce_different_ciphertexts() {
        let dir_a = TestDir::new();
        let dir_b = TestDir::new();

        let enc_a = Encryptor::new(dir_a.path()).unwrap();
        let enc_b = Encryptor::new(dir_b.path()).unwrap();

        let config = ProviderConfig {
            name: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            api_key: "sk-test-key".to_string(),
            model: "gpt-4".to_string(),
        };

        let encrypted_a = config.encrypt_api_key(&enc_a).unwrap();
        let encrypted_b = config.encrypt_api_key(&enc_b).unwrap();

        // Different keys should produce different ciphertexts
        assert_ne!(encrypted_a, encrypted_b);

        // Decrypting with wrong key should fail
        let mut config2 = config.clone();
        let result = config2.decrypt_api_key(&enc_b, &encrypted_a);
        assert!(result.is_err());
    }
}
