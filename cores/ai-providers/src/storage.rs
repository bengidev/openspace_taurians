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
///
/// When `api_key` is `None`, the provider is stored without an encrypted key
/// (used for seed/default profiles). Users can set a key later via `update`.
#[derive(Debug, Clone)]
pub struct NewProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub auth_header_name: Option<String>,
    pub auth_header_value_prefix: Option<String>,
    pub models: Vec<ModelInfo>,
    pub request_body_template: serde_json::Value,
    pub response_path: String,
}

/// Fields accepted when updating a provider.
///
/// `api_key` semantics:
/// - `Some(new_key)` → encrypt and store the new key
/// - `None` → preserve the existing key
/// - `Some("")` → clear the key (set to NULL)
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

/// The currently active provider and model choice, persisted across restarts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ActiveProvider {
    pub provider_id: i64,
    pub model: String,
}

/// SQLite-backed provider configuration store.
pub struct ProviderStore {
    conn: Connection,
    encryptor: Encryptor,
}

impl ProviderStore {
    /// Open a provider store at `database_path` and create required schema.
    pub fn open(
        database_path: impl AsRef<Path>,
        data_dir: impl AsRef<Path>,
    ) -> Result<Self, ProviderStoreError> {
        if let Some(parent) = database_path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(EncryptionError::Io)?;
        }

        let conn = Connection::open(database_path)?;
        Self::enable_foreign_keys(&conn)?;
        let encryptor = Encryptor::new(data_dir.as_ref())?;
        let store = Self { conn, encryptor };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory provider store for tests.
    pub fn in_memory(data_dir: impl AsRef<Path>) -> Result<Self, ProviderStoreError> {
        let conn = Connection::open_in_memory()?;
        Self::enable_foreign_keys(&conn)?;
        let encryptor = Encryptor::new(data_dir.as_ref())?;
        let store = Self { conn, encryptor };
        store.migrate()?;
        Ok(store)
    }

    fn enable_foreign_keys(conn: &Connection) -> Result<(), ProviderStoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(())
    }

    /// Apply the providers table migration.
    pub fn migrate(&self) -> Result<(), ProviderStoreError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted BLOB,
                auth_header_name TEXT NOT NULL DEFAULT 'Authorization',
                auth_header_value_prefix TEXT NOT NULL DEFAULT 'Bearer ',
                models TEXT NOT NULL,
                request_body_template TEXT NOT NULL,
                response_path TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS active_provider (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                provider_id INTEGER NOT NULL,
                model TEXT NOT NULL,
                FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
            );
            "#,
        )?;
        // Migrate existing databases that had NOT NULL constraint on api_key_encrypted.
        self.migrate_nullable_api_key()?;
        Ok(())
    }

    /// If the existing `providers` table has a NOT NULL constraint on
    /// `api_key_encrypted`, recreate it with a nullable column.
    fn migrate_nullable_api_key(&self) -> Result<(), ProviderStoreError> {
        let column_notnull: i32 = self
            .conn
            .query_row(
                "SELECT [notnull] FROM pragma_table_info('providers') WHERE name = 'api_key_encrypted'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0); // default to 0 (nullable) if query fails

        if column_notnull == 0 {
            return Ok(());
        }

        // Recreate with nullable column, preserving all existing data.
        self.conn.execute_batch(
            r#"
            CREATE TABLE providers_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                base_url TEXT NOT NULL,
                api_key_encrypted BLOB,
                auth_header_name TEXT NOT NULL DEFAULT 'Authorization',
                auth_header_value_prefix TEXT NOT NULL DEFAULT 'Bearer ',
                models TEXT NOT NULL,
                request_body_template TEXT NOT NULL,
                response_path TEXT NOT NULL
            );
            INSERT INTO providers_new SELECT * FROM providers;
            DROP TABLE providers;
            ALTER TABLE providers_new RENAME TO providers;
            "#,
        )?;
        Ok(())
    }

    /// Create a provider and return its generated id.
    ///
    /// When `api_key` is `None` (or empty), `api_key_encrypted` is stored as NULL.
    pub fn create(&self, input: NewProviderConfig) -> Result<i64, ProviderStoreError> {
        let encrypted_key: Option<Vec<u8>> = match input.api_key {
            Some(ref key) if !key.is_empty() => Some(self.encryptor.encrypt(key.as_bytes())?),
            _ => None,
        };
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
        // Check if the provider exists.
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) FROM providers WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if !exists {
            return Ok(false);
        }

        let existing_key: Option<Vec<u8>> = self.conn.query_row(
            "SELECT api_key_encrypted FROM providers WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;

        let encrypted_key: Option<Vec<u8>> = match input.api_key {
            Some(ref key) if !key.is_empty() => Some(self.encryptor.encrypt(key.as_bytes())?),
            Some(_) => None,      // empty string → clear key
            None => existing_key, // None → preserve existing
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
    ///
    /// If the deleted provider was the active selection, the active state is
    /// cleared as well (enforced by `ON DELETE CASCADE`).
    pub fn delete(&self, id: i64) -> Result<bool, ProviderStoreError> {
        let was_active = self
            .get_active()?
            .is_some_and(|a| a.provider_id == id);
        let deleted = self
            .conn
            .execute("DELETE FROM providers WHERE id = ?1", params![id])?
            > 0;
        if deleted && was_active {
            // Safety: clear active even though CASCADE should handle it.
            self.clear_active()?;
        }
        Ok(deleted)
    }

    /// Decrypt an encrypted provider API key for internal callers/tests.
    ///
    /// Returns `Ok(None)` when the provider has no API key (seed profile).
    pub fn decrypt_api_key(
        &self,
        provider: &ProviderConfig,
    ) -> Result<Option<String>, ProviderStoreError> {
        provider
            .decrypt_api_key(&self.encryptor)
            .map_err(Into::into)
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

    /// Check if a provider with the given name already exists.
    fn provider_exists(&self, name: &str) -> Result<bool, ProviderStoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM providers WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Seed default (key-less) provider profiles into the database.
    ///
    /// Idempotent: profiles are only inserted when no profile with the same
    /// name exists.
    pub fn seed_default_profiles(&self) -> Result<usize, ProviderStoreError> {
        let mut seeded = 0;
        for profile in crate::config::default_profiles::all() {
            if !self.provider_exists(&profile.name)? {
                self.create(profile)?;
                seeded += 1;
            }
        }
        Ok(seeded)
    }

    fn provider_from_row(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> Result<ProviderConfig, rusqlite::Error> {
        let models_json: String = row.get(6)?;
        let request_body_template_json: String = row.get(7)?;

        let models = serde_json::from_str(&models_json).map_err(json_to_sql_error)?;
        let request_body_template =
            serde_json::from_str(&request_body_template_json).map_err(json_to_sql_error)?;

        Ok(ProviderConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            base_url: row.get(2)?,
            api_key_encrypted: row.get::<_, Option<Vec<u8>>>(3)?,
            auth_header_name: row.get(4)?,
            auth_header_value_prefix: row.get(5)?,
            models,
            request_body_template,
            response_path: row.get(8)?,
        })
    }

    /// Get the currently active provider/model selection, or `None` if unset.
    pub fn get_active(&self) -> Result<Option<ActiveProvider>, ProviderStoreError> {
        let result: Option<(i64, String)> = self
            .conn
            .query_row(
                "SELECT provider_id, model FROM active_provider WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(result.map(|(provider_id, model)| ActiveProvider {
            provider_id,
            model,
        }))
    }

    /// Persist the active provider and model. Replaces any prior selection.
    pub fn set_active(
        &self,
        provider_id: i64,
        model: &str,
    ) -> Result<(), ProviderStoreError> {
        // Verify the provider exists.
        let provider = self.get(provider_id)?;
        if provider.is_none() {
            return Err(ProviderStoreError::Adapter(format!(
                "provider '{provider_id}' not found"
            )));
        }

        // Verify the model is valid for this provider.
        let valid_model = provider
            .as_ref()
            .unwrap()
            .models
            .iter()
            .any(|m| m.id == model);
        if !valid_model {
            return Err(ProviderStoreError::Adapter(format!(
                "model '{model}' not available for provider '{provider_id}'"
            )));
        }

        self.conn.execute(
            "INSERT INTO active_provider (id, provider_id, model) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET provider_id = ?1, model = ?2",
            params![provider_id, model],
        )?;
        Ok(())
    }

    /// Clear the active provider/model selection (e.g. on provider deletion).
    pub fn clear_active(&self) -> Result<bool, ProviderStoreError> {
        Ok(self.conn.execute("DELETE FROM active_provider WHERE id = 1", [])? > 0)
    }

    fn ensure_decryptable(
        &self,
        provider: ProviderConfig,
    ) -> Result<ProviderConfig, ProviderStoreError> {
        // Seed profiles (no key) skip decryption verification.
        if provider.has_api_key() {
            let _ = self.decrypt_api_key(&provider)?;
        }
        Ok(provider)
    }
}

fn json_to_sql_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
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
            api_key: Some(api_key.to_string()),
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
        assert!(provider
            .api_key_encrypted
            .as_ref()
            .is_some_and(|v| !v.is_empty()));
        assert!(provider.has_api_key());
        assert_eq!(
            store.decrypt_api_key(&provider).unwrap(),
            Some("sk-secret".to_string())
        );

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
        assert_eq!(
            store.decrypt_api_key(&provider).unwrap(),
            Some("sk-new-secret".to_string())
        );

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
        assert_eq!(
            store.decrypt_api_key(&provider).unwrap(),
            Some("sk-preserved".to_string())
        );
    }

    #[test]
    fn seed_default_profiles_inserts_all_and_is_idempotent() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        // First seed: inserts 3 profiles
        let seeded = store.seed_default_profiles().unwrap();
        assert_eq!(seeded, 3);

        // Verify all three are present
        let providers = store.list().unwrap();
        assert_eq!(providers.len(), 3);

        let names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"OpenAI"));
        assert!(names.contains(&"Anthropic"));
        assert!(names.contains(&"OpenRouter"));

        // All have no API key
        for provider in &providers {
            assert!(!provider.has_api_key());
            assert_eq!(provider.api_key_encrypted, None);
        }

        // OpenAI specific checks
        let openai = providers.iter().find(|p| p.name == "OpenAI").unwrap();
        assert_eq!(openai.auth_header_name, "Authorization");
        assert_eq!(openai.auth_header_value_prefix, "Bearer ");
        assert!(openai.base_url.ends_with("/chat/completions"));
        assert!(openai.models.iter().any(|m| m.id == "gpt-4o"));
        assert_eq!(openai.response_path, "choices[0].message.content");

        // Anthropic specific checks
        let anthropic = providers.iter().find(|p| p.name == "Anthropic").unwrap();
        assert_eq!(anthropic.auth_header_name, "x-api-key");
        assert_eq!(anthropic.auth_header_value_prefix, "");
        assert!(anthropic.models.iter().any(|m| m.id.contains("claude")));
        assert_eq!(anthropic.response_path, "content[0].text");

        // OpenRouter specific checks
        let openrouter = providers.iter().find(|p| p.name == "OpenRouter").unwrap();
        assert_eq!(openrouter.auth_header_name, "Authorization");
        assert!(openrouter.models.iter().all(|m| m.id.contains('/')));

        // Second seed: nothing inserted (idempotent)
        let seeded_again = store.seed_default_profiles().unwrap();
        assert_eq!(seeded_again, 0);
        assert_eq!(store.list().unwrap().len(), 3);
    }

    #[test]
    fn seed_default_profiles_does_not_overwrite_user_data() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        // User creates their own OpenAI provider with a key first
        let id = store.create(new_provider("sk-user-key")).unwrap();

        // Seed should skip OpenAI (already exists) but insert Anthropic + OpenRouter
        let seeded = store.seed_default_profiles().unwrap();
        assert_eq!(seeded, 2);

        // User's OpenAI is preserved
        let user_provider = store.get(id).unwrap().unwrap();
        assert_eq!(user_provider.name, "OpenAI");
        assert!(user_provider.has_api_key());
        assert_eq!(
            store.decrypt_api_key(&user_provider).unwrap(),
            Some("sk-user-key".to_string())
        );
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

    #[test]
    fn active_provider_get_returns_none_when_unset() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        assert!(store.get_active().unwrap().is_none());
    }

    #[test]
    fn active_provider_set_and_get_roundtrip() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id = store.create(new_provider("sk-key")).unwrap();
        store.set_active(id, "gpt-4o").unwrap();

        let active = store.get_active().unwrap().unwrap();
        assert_eq!(active.provider_id, id);
        assert_eq!(active.model, "gpt-4o");
    }

    #[test]
    fn active_provider_set_replaces_previous() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id1 = store.create(new_provider("sk-key1")).unwrap();
        let mut input2 = new_provider("sk-key2");
        input2.name = "Anthropic".to_string();
        input2.models = vec![ModelInfo {
            id: "claude-3-opus".to_string(),
            name: "Claude 3 Opus".to_string(),
            context_window: 200000,
        }];
        let id2 = store.create(input2).unwrap();

        store.set_active(id1, "gpt-4o").unwrap();
        store.set_active(id2, "claude-3-opus").unwrap();

        let active = store.get_active().unwrap().unwrap();
        assert_eq!(active.provider_id, id2);
        assert_eq!(active.model, "claude-3-opus");
    }

    #[test]
    fn active_provider_set_fails_for_nonexistent_provider() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let result = store.set_active(999, "gpt-4o");
        assert!(result.is_err());
    }

    #[test]
    fn active_provider_set_fails_for_nonexistent_model() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id = store.create(new_provider("sk-key")).unwrap();
        let result = store.set_active(id, "nonexistent-model");
        assert!(result.is_err());
    }

    #[test]
    fn active_provider_clear_removes_selection() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id = store.create(new_provider("sk-key")).unwrap();
        store.set_active(id, "gpt-4o").unwrap();
        assert!(store.get_active().unwrap().is_some());

        store.clear_active().unwrap();
        assert!(store.get_active().unwrap().is_none());
    }

    #[test]
    fn delete_active_provider_clears_active() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id = store.create(new_provider("sk-key")).unwrap();
        store.set_active(id, "gpt-4o").unwrap();

        store.delete(id).unwrap();
        assert!(store.get_active().unwrap().is_none());
    }

    #[test]
    fn delete_inactive_provider_preserves_active() {
        let test_dir = TestDir::new();
        let store = ProviderStore::in_memory(test_dir.path()).unwrap();

        let id1 = store.create(new_provider("sk-key1")).unwrap();
        let mut input2 = new_provider("sk-key2");
        input2.name = "Anthropic".to_string();
        let id2 = store.create(input2).unwrap();

        store.set_active(id1, "gpt-4o").unwrap();
        store.delete(id2).unwrap();

        let active = store.get_active().unwrap().unwrap();
        assert_eq!(active.provider_id, id1);
        assert_eq!(active.model, "gpt-4o");
    }
}
