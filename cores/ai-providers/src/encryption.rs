//! Encryption module for API key encryption at rest.
//!
//! Provides device-local encryption using AES-256-GCM.
//! The encryption key is derived from a random seed file stored in the app data directory.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use std::path::Path;
use thiserror::Error;

/// Name of the seed file stored in the app data directory.
const SEED_FILE_NAME: &str = ".encryption_seed";

/// Length of the encryption key in bytes (256 bits for AES-256).
const KEY_LEN: usize = 32;

/// Length of the nonce in bytes (96 bits for AES-GCM).
const NONCE_LEN: usize = 12;

/// Errors that can occur during encryption/decryption operations.
#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed")]
    DecryptionFailed,

    #[error("Invalid ciphertext: too short")]
    InvalidCiphertext,
}

/// Generate cryptographically secure random bytes.
fn generate_random_bytes() -> [u8; KEY_LEN] {
    let mut buf = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

/// Load or create the encryption key from a seed file in the given data directory.
///
/// If the seed file doesn't exist, it will be created with a new random key.
/// On Unix systems, the file is created with restrictive permissions (0o600).
///
/// # Arguments
///
/// * `data_dir` - The directory where the seed file should be stored
///
/// # Returns
///
/// The 32-byte encryption key, or an error if I/O operations fail.
pub fn get_or_create_key(data_dir: &Path) -> Result<[u8; KEY_LEN], EncryptionError> {
    // Ensure the data directory exists
    std::fs::create_dir_all(data_dir)?;

    let seed_path = data_dir.join(SEED_FILE_NAME);

    if seed_path.exists() {
        let bytes = std::fs::read(&seed_path)?;
        if bytes.len() == KEY_LEN {
            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            Ok(key)
        } else {
            // Regenerate if seed file is corrupted
            let key = generate_random_bytes();
            std::fs::write(&seed_path, &key)?;
            Ok(key)
        }
    } else {
        let key = generate_random_bytes();
        std::fs::write(&seed_path, &key)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&seed_path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(key)
    }
}
