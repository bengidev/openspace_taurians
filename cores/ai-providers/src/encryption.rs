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

/// Encrypt plaintext using AES-256-GCM with a device-local key.
///
/// The encrypted output format is: nonce (12 bytes) + ciphertext.
/// The key is derived from a seed file in the data directory.
///
/// # Arguments
///
/// * `data_dir` - The directory where the seed file is stored
/// * `plaintext` - The data to encrypt
///
/// # Returns
///
/// The encrypted data (nonce + ciphertext), or an error if encryption fails.
pub fn encrypt(data_dir: &Path, plaintext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    let key = get_or_create_key(data_dir)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    // Generate a random nonce
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt the plaintext
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| EncryptionError::EncryptionFailed)?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt ciphertext using AES-256-GCM with a device-local key.
///
/// Expects the input format: nonce (12 bytes) + ciphertext.
/// The key is derived from a seed file in the data directory.
///
/// # Arguments
///
/// * `data_dir` - The directory where the seed file is stored
/// * `ciphertext` - The encrypted data (nonce + ciphertext)
///
/// # Returns
///
/// The decrypted plaintext, or an error if decryption fails.
pub fn decrypt(data_dir: &Path, ciphertext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    if ciphertext.len() < NONCE_LEN {
        return Err(EncryptionError::InvalidCiphertext);
    }

    let key = get_or_create_key(data_dir)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));

    // Split nonce and encrypted data
    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt the ciphertext
    cipher
        .decrypt(nonce, encrypted)
        .map_err(|_| EncryptionError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "encryption_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let test_dir = create_test_dir();
        let plaintext = b"Hello, World!";

        let encrypted = encrypt(&test_dir, plaintext).unwrap();
        let decrypted = decrypt(&test_dir, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);

        // Cleanup
        fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_encrypt_decrypt_empty_data() {
        let test_dir = create_test_dir();
        let plaintext = b"";

        let encrypted = encrypt(&test_dir, plaintext).unwrap();
        let decrypted = decrypt(&test_dir, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);

        // Cleanup
        fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_encrypt_decrypt_large_data() {
        let test_dir = create_test_dir();
        let plaintext = vec![42u8; 10_000];

        let encrypted = encrypt(&test_dir, &plaintext).unwrap();
        let decrypted = decrypt(&test_dir, &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);

        // Cleanup
        fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_decrypt_invalid_ciphertext() {
        let test_dir = create_test_dir();
        let short_ciphertext = vec![0u8; 5]; // Too short

        let result = decrypt(&test_dir, &short_ciphertext);
        assert!(matches!(result, Err(EncryptionError::InvalidCiphertext)));

        // Cleanup
        fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        let test_dir = create_test_dir();
        let plaintext = b"Secret data";

        let mut encrypted = encrypt(&test_dir, plaintext).unwrap();
        // Tamper with the ciphertext
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt(&test_dir, &encrypted);
        assert!(matches!(result, Err(EncryptionError::DecryptionFailed)));

        // Cleanup
        fs::remove_dir_all(&test_dir).ok();
    }
}
