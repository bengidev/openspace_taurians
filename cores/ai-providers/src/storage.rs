//! SQLite persistence for provider configurations.

use crate::config::{
    default_auth_header_name, default_auth_header_value_prefix, ModelInfo, ProviderConfig,
};
use crate::encryption::{EncryptionError, Encryptor};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use thiserror::Error;

/// Errors returned by provider persistence operations.
#[derive(Debug, Error)]
pub enum ProviderStoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("encryption error: {0}")]
    Encryption(#[from] EncryptionError),

    #[error("adapter error: {0}")]
    Adapter(String),
}

/// Fields accepted when creating a provider.
#[derive(Debug, Clone)]
pub struct NewProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub auth_header_name: Option<String>,
    pub auth_header_value_prefix: Option<String>,
    pub models: Vec<ModelInfo>,
    pub request_body_template: serde_json::Value,
    pub response_path: String,
}

/// Fields accepted when updating a provider.
#[derive(Debug, Clone)]
pub struct UpdateProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub auth_header_name: Option<String>,
    pub auth_header_value_prefix: Option<String>,
    pub models: Vec<ModelInfo>,
    pub request_body_template: serde_json::Value,
    pub response_path: String,
}

/// SQLite-backed provider configuration store.
pub struct ProviderStore {
    conn: Connection,
    encryptor: Encryptor,
}

impl ProviderStore {
    /// Open a provider store at `database_path` and create required schema.
    pub fn open(database_path: impl AsRef<Path>, data_dir: impl AsRef<Path>) -> Result<Self, ProviderStoreError> {
        if let Some(parent) = database_path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(EncryptionError::Io)?;
        }

        let conn = Connection::open(database_path)?;
        let encryptor = Encryptor::new(data_dir.as_ref())?;
        let store = Self { conn, encryptor };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory provider store for tests.
    pub fn in_memory(data_dir: impl AsRef<Path>) -> Result<Self, ProviderStoreError> {
        let conn = Connection::open_in_memory()?;
        let encryptor = Encryptor::new(data_dir.as_ref())?;
        let store = Self { conn, encryptor };
        store.migrate()?;
        Ok(store)
    }

    /// Apply the providers table migration.
    pub fn migrate(&self) -> Result<(), ProviderStoreError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted BLOB NOT NULL,
                auth_header_name TEXT NOT NULL DEFAULT 'Authorization',
                auth_header_value_prefix TEXT NOT NULL DEFAULT 'Bearer ',
                models TEXT NOT NULL,
                request_body_template TEXT NOT NULL,
                response_path TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    /// Create a provider and return its generated id.
    pub fn create(&self, input: NewProviderConfig) -> Result<i64, ProviderStoreError> {
        let encrypted_key = self.encryptor.encrypt(input.api_key.as_bytes())?;
        let models_json = serde_json::to_string(&input.models)?;
        let template_json = serde_json::to_string(&input.request_body_template)?;
        let auth_header_name = input
            .auth_header_name
            .unwrap_or_else(default_auth_header_name);
        let auth_header_value_prefix = input
            .auth_header_value_prefix
            .unwrap_or_else(default_auth_header_value_prefix);

        self.conn.execute(
            r#"
            INSERT INTO providers (
                name,
                base_url,
                api_key_encrypted,
                auth_header_name,
                auth_header_value_prefix,
                models,
                request_body_template,
                response_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                input.name,
                input.base_url,
                encrypted_key,
                auth_header_name,
                auth_header_value_prefix,
                models_json,
                template_json,
                input.response_path,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    /// Get one provider by id, decrypting its API key into memory to verify readability.
    pub fn get(&self, id: i64) -> Result<Option<ProviderConfig>, ProviderStoreError> {
        self.conn
            .query_row(
                r#"
                SELECT
                    id,
                    name,
                    base_url,
                    api_key_encrypted,
                    auth_header_name,
                    auth_header_value_prefix,
                    models,
                    request_body_template,
                    response_path
                FROM providers
                WHERE id = ?1
                "#,
                params![id],
                |row| self.provider_from_row(row),
            )
            .optional()?
            .map(|provider| self.ensure_decryptable(provider))
            .transpose()
    }

    /// List all providers, decrypting API keys into memory to verify readability.
    pub fn list(&self) -> Result<Vec<ProviderConfig>, ProviderStoreError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
                id,
                name,
                base_url,
                api_key_encrypted,
                auth_header_name,
                auth_header_value_prefix,
                models,
                request_body_template,
                response_path
            FROM providers
            ORDER BY id
            "#,
        )?;

        let providers = stmt
            .query_map([], |row| self.provider_from_row(row))?
            .collect::<Result<Vec<_>, _>>()?;

        providers
            .into_iter()
            .map(|provider| self.ensure_decryptable(provider))
            .collect()
    }

    /// Update a provider. Returns `true` when a row was updated.
    pub fn update(&self, id: i64, input: UpdateProviderConfig) -> Result<bool, ProviderStoreError> {
        let existing_key: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT api_key_encrypted FROM providers WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        let Some(existing_key) = existing_key else {
            return Ok(false);
        };

        let encrypted_key = match input.api_key {
            Some(api_key) => self.encryptor.encrypt(api_key.as_bytes())?,
            None => existing_key,
        };
        let models_json = serde_json::to_string(&input.models)?;
        let template_json = serde_json::to_string(&input.request_body_template)?;
        let auth_header_name = input
            .auth_header_name
            .unwrap_or_else(default_auth_header_name);
        let auth_header_value_prefix = input
            .auth_header_value_prefix
            .unwrap_or_else(default_auth_header_value_prefix);

        let changed = self.conn.execute(
            r#"
            UPDATE providers
            SET
                name = ?1,
                base_url = ?2,
                api_key_encrypted = ?3,
                auth_header_name = ?4,
                auth_header_value_prefix = ?5,
                models = ?6,
                request_body_template = ?7,
                response_path = ?8
            WHERE id = ?9
            "#,
            params![
                input.name,
                input.base_url,
                encrypted_key,
                auth_header_name,
                auth_header_value_prefix,
                models_json,
                template_json,
                input.response_path,
                id,
            ],
        )?;

        Ok(changed > 0)
    }

    /// Delete a provider. Returns `true` when a row was deleted.
    pub fn delete(&self, id: i64) -> Result<bool, ProviderStoreError> {
        Ok(self
            .conn
            .execute("DELETE FROM providers WHERE id = ?1", params![id])?
            > 0)
    }

    /// Decrypt an encrypted provider API key for internal callers/tests.
    pub fn decrypt_api_key(&self, provider: &ProviderConfig) -> Result<String, ProviderStoreError> {
        provider.decrypt_api_key(&self.encryptor).map_err(Into::into)
    }

    /// Build a generic HTTP adapter for a persisted provider.
    pub fn ai_provider(&self, id: i64) -> Result<Option<crate::AiProvider>, ProviderStoreError> {
        self.get(id)?
            .map(|provider| crate::AiProvider::new(provider, &self.encryptor))
            .transpose()
            .map_err(|error| match error {
                crate::AiProviderError::Encryption(error) => ProviderStoreError::Encryption(error),
                other => ProviderStoreError::Adapter(other.to_string()),
            })
    }

    fn provider_from_row(&self, row: &rusqlite::Row<'_>) -> Result<ProviderConfig, rusqlite::Error> {
        let models_json: String = row.get(6)?;
        let request_body_template_json: String = row.get(7)?;

        let models = serde_json::from_str(&models_json).map_err(json_to_sql_error)?;
        let request_body_template =
            serde_json::from_str(&request_body_template_json).map_err(json_to_sql_error)?;

        Ok(ProviderConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key_encrypted: row.get(3)?,
            auth_header_name: row.get(4)?,
            auth_header_value_prefix: row.get(5)?,
            models,
            request_body_template,
            response_path: row.get(8)?,
        })
    }

    fn ensure_decryptable(&self, provider: ProviderConfig) -> Result<ProviderConfig, ProviderStoreError> {
        let _ = self.decrypt_api_key(&provider)?;
        Ok(provider)
    }
}

fn json_to_sql_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(error),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::path::{Path, PathBuf};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let mut rng = rand::thread_rng();
            let dir = std::env::temp_dir().join(format!(
                "provider_store_test_{}_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                rng.gen::<u64>()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self { path: dir }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn model() -> ModelInfo {
        ModelInfo {
            id: "gpt-4o".to_string(),
            name: "GPT-4o".to_string(),
            context_window: 128000,
        }
    }

    fn new_provider(api_key: &str) -> NewProviderConfig {
        NewProviderConfig {
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: api_key.to_string(),
            auth_header_name: None,
            auth_header_value_prefix: None,
            models: vec![model()],
            request_body_template: serde_json::json!({
                "model": "{model}",
                "messages": "{messages}",
                "stream": "{stream}",
                "temperature": "{temperature}"
            }),
            response_path: "choices[0].message.content".to_string(),
        }
    }

    #[test]
    fn create_get_list_update_delete_provider() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id = store.create(new_provider("sk-secret")).unwrap();
        assert!(id > 0);

        let provider = store.get(id).unwrap().unwrap();
        assert_eq!(provider.name, "OpenAI");
        assert_eq!(provider.auth_header_name, "Authorization");
        assert_eq!(provider.auth_header_value_prefix, "Bearer ");
        assert_eq!(provider.models, vec![model()]);
        assert_ne!(provider.api_key_encrypted, b"sk-secret");
        assert_eq!(store.decrypt_api_key(&provider).unwrap(), "sk-secret");

        let providers = store.list().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, id);

        let updated = store
            .update(
                id,
                UpdateProviderConfig {
                    name: "Anthropic".to_string(),
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: Some("sk-new-secret".to_string()),
                    auth_header_name: Some("X-API-Key".to_string()),
                    auth_header_value_prefix: Some(String::new()),
                    models: vec![ModelInfo {
                        id: "claude-3-5-sonnet".to_string(),
                        name: "Claude 3.5 Sonnet".to_string(),
                        context_window: 200000,
                    }],
                    request_body_template: serde_json::json!({"model": "{model}"}),
                    response_path: "content[0].text".to_string(),
                },
            )
            .unwrap();
        assert!(updated);

        let provider = store.get(id).unwrap().unwrap();
        assert_eq!(provider.name, "Anthropic");
        assert_eq!(provider.auth_header_name, "X-API-Key");
        assert_eq!(provider.auth_header_value_prefix, "");
        assert_eq!(store.decrypt_api_key(&provider).unwrap(), "sk-new-secret");

        assert!(store.delete(id).unwrap());
        assert!(store.get(id).unwrap().is_none());
        assert!(!store.delete(id).unwrap());
    }

    #[test]
    fn update_without_api_key_preserves_existing_encrypted_key() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();
        let id = store.create(new_provider("sk-preserved")).unwrap();
        let before = store.get(id).unwrap().unwrap().api_key_encrypted;

        assert!(store
            .update(
                id,
                UpdateProviderConfig {
                    name: "OpenAI renamed".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: None,
                    auth_header_name: None,
                    auth_header_value_prefix: None,
                    models: vec![model()],
                    request_body_template: serde_json::json!({"model": "{model}"}),
                    response_path: "choices[0].message.content".to_string(),
                },
            )
            .unwrap());

        let provider = store.get(id).unwrap().unwrap();
        assert_eq!(provider.api_key_encrypted, before);
        assert_eq!(store.decrypt_api_key(&provider).unwrap(), "sk-preserved");
    }

    #[test]
    fn update_missing_provider_returns_false() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let updated = store
            .update(
                99,
                UpdateProviderConfig {
                    name: "Missing".to_string(),
                    base_url: "https://api.example.com".to_string(),
                    api_key: None,
                    auth_header_name: None,
                    auth_header_value_prefix: None,
                    models: vec![],
                    request_body_template: serde_json::json!({}),
                    response_path: "content".to_string(),
                },
            )
            .unwrap();

        assert!(!updated);
    }
}
