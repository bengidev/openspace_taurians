//! Feature registry — manages feature metadata, routes commands
//! between features, and tracks panel lifecycle state.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── FeatureId ────────────────────────────────────────────────────

/// Unique identifier for a registered feature.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureId(String);

impl FeatureId {
    /// Create a new `FeatureId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FeatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for FeatureId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for FeatureId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ── FeatureMetadata ──────────────────────────────────────────────

/// Metadata describing a registered feature.
///
/// Includes the feature's identity, display name, icon, and the
/// path to its capability declaration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub id: FeatureId,
    pub name: String,
    pub icon: String,
    pub capability_file: PathBuf,
}

// ── RegistryError ────────────────────────────────────────────────

/// Errors returned by [`FeatureRegistry`] operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("feature with id '{0}' is already registered")]
    DuplicateRegistration(FeatureId),

    #[error("feature with id '{0}' not found")]
    NotFound(FeatureId),
}

// ── FeatureRegistry ──────────────────────────────────────────────

/// Central registry of feature metadata.
///
/// Stores metadata for all registered features and provides
/// lookup by `FeatureId`. Rejects duplicate registrations.
pub struct FeatureRegistry {
    features: HashMap<FeatureId, FeatureMetadata>,
}

impl FeatureRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }

    /// Register a feature. Returns [`RegistryError::DuplicateRegistration`]
    /// if a feature with the same `FeatureId` already exists.
    pub fn register(&mut self, meta: FeatureMetadata) -> Result<(), RegistryError> {
        if self.features.contains_key(&meta.id) {
            return Err(RegistryError::DuplicateRegistration(meta.id));
        }
        self.features.insert(meta.id.clone(), meta);
        Ok(())
    }

    /// Remove a feature by id. Returns [`RegistryError::NotFound`]
    /// if no feature with the given id is registered.
    pub fn unregister(&mut self, id: &FeatureId) -> Result<(), RegistryError> {
        self.features
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| RegistryError::NotFound(id.clone()))
    }

    /// Look up a feature by id. Returns `None` if not found.
    pub fn get(&self, id: &FeatureId) -> Option<&FeatureMetadata> {
        self.features.get(id)
    }

    /// Return all registered features.
    pub fn list(&self) -> Vec<&FeatureMetadata> {
        self.features.values().collect()
    }
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}
