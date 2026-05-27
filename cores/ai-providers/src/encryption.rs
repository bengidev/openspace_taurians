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

    #[error(
        "Encryption seed file is corrupted (wrong length). \
         All previously encrypted data would be unrecoverable if regenerated."
    )]
    SeedCorrupted,
}

/// Generate cryptographically secure random bytes of length `N`.
fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

/// Load or create the encryption key from a seed file in the given data directory.
///
/// If the seed file doesn't exist, it will be created with a new random key.
/// On Unix systems, the file is created atomically with restrictive permissions (0o600).
///
/// If the seed file exists but is the wrong length, [`EncryptionError::SeedCorrupted`]
/// is returned to prevent accidental data loss — regenerating the key would make
/// all previously encrypted data unrecoverable.
///
/// # Arguments
///
/// * `data_dir` - The directory where the seed file should be stored
///
/// # Returns
///
/// The 32-byte encryption key, or an error if I/O operations fail
/// or the seed file is corrupted.
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
            Err(EncryptionError::SeedCorrupted)
        }
    } else {
        let key = random_bytes::<KEY_LEN>();
        create_seed_file(&seed_path, &key)?;
        Ok(key)
    }
}

/// Create the seed file with the given key bytes.
///
/// On Unix, the file is created with mode 0o600 atomically via [`OpenOptions`],
/// avoiding the TOCTOU window between `write` and `chmod`.
#[cfg(unix)]
fn create_seed_file(path: &Path, key: &[u8; KEY_LEN]) -> Result<(), EncryptionError> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(key)?;
    Ok(())
}

/// Create the seed file with the given key bytes.
///
/// On non-Unix platforms (Windows), file permissions are not hardened —
/// this is a known platform limitation.
#[cfg(not(unix))]
fn create_seed_file(path: &Path, key: &[u8; KEY_LEN]) -> Result<(), EncryptionError> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(key)?;
    Ok(())
}

/// Device-local encryptor that loads the encryption key once from the seed file.
///
/// Prefer this struct over the free functions when performing multiple
/// encrypt/decrypt operations — it avoids reading the seed file from disk
/// on every call and eliminates races on concurrent seed-file access.
pub struct Encryptor {
    key: [u8; KEY_LEN],
}

impl Encryptor {
    /// Create a new encryptor, loading the key from the seed file in `data_dir`.
    ///
    /// If the seed file doesn't exist, it will be created.
    pub fn new(data_dir: &Path) -> Result<Self, EncryptionError> {
        Ok(Self {
            key: get_or_create_key(data_dir)?,
        })
    }

    /// Encrypt plaintext using AES-256-GCM.
    ///
    /// The encrypted output format is: nonce (12 bytes) + ciphertext.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

        let nonce_bytes = random_bytes::<NONCE_LEN>();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| EncryptionError::EncryptionFailed)?;

        let mut result = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);
        Ok(result)
    }

    /// Decrypt ciphertext using AES-256-GCM.
    ///
    /// Expects the input format: nonce (12 bytes) + ciphertext.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        if ciphertext.len() < NONCE_LEN {
            return Err(EncryptionError::InvalidCiphertext);
        }

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

        let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| EncryptionError::DecryptionFailed)
    }
}

/// Encrypt plaintext using AES-256-GCM with a device-local key.
///
/// Convenience wrapper around [`Encryptor`] for one-shot operations.
/// For multiple operations, construct an [`Encryptor`] to avoid
/// reading the seed file on every call.
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
    let encryptor = Encryptor::new(data_dir)?;
    encryptor.encrypt(plaintext)
}

/// Decrypt ciphertext using AES-256-GCM with a device-local key.
///
/// Convenience wrapper around [`Encryptor`] for one-shot operations.
/// For multiple operations, construct an [`Encryptor`] to avoid
/// reading the seed file on every call.
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
    let encryptor = Encryptor::new(data_dir)?;
    encryptor.decrypt(ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// RAII guard that removes the test directory on drop, including on panic.
    struct TestDir {
        path: std::path::PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let dir = std::env::temp_dir().join(format!(
                "encryption_test_{}_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                rng.gen::<u64>()
            ));
            fs::create_dir_all(&dir).unwrap();
            Self { path: dir }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // --- Free function tests (convenience wrappers) ---

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let test_dir = TestDir::new();
        let plaintext = b"Hello, World!";

        let encrypted = encrypt(test_dir.path(), plaintext).unwrap();
        let decrypted = decrypt(test_dir.path(), &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty_data() {
        let test_dir = TestDir::new();
        let plaintext = b"";

        let encrypted = encrypt(test_dir.path(), plaintext).unwrap();
        let decrypted = decrypt(test_dir.path(), &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large_data() {
        let test_dir = TestDir::new();
        let plaintext = vec![42u8; 10_000];

        let encrypted = encrypt(test_dir.path(), &plaintext).unwrap();
        let decrypted = decrypt(test_dir.path(), &encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_ciphertext() {
        let test_dir = TestDir::new();
        let short_ciphertext = vec![0u8; 5]; // Too short

        let result = decrypt(test_dir.path(), &short_ciphertext);
        assert!(matches!(result, Err(EncryptionError::InvalidCiphertext)));
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        let test_dir = TestDir::new();
        let plaintext = b"Secret data";

        let mut encrypted = encrypt(test_dir.path(), plaintext).unwrap();
        // Tamper with the ciphertext
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt(test_dir.path(), &encrypted);
        assert!(matches!(result, Err(EncryptionError::DecryptionFailed)));
    }

    #[test]
    fn test_different_data_dir_fails() {
        let dir_a = TestDir::new();
        let dir_b = TestDir::new();
        let plaintext = b"Confidential data";

        let encrypted = encrypt(dir_a.path(), plaintext).unwrap();
        let result = decrypt(dir_b.path(), &encrypted);

        assert!(matches!(result, Err(EncryptionError::DecryptionFailed)));
    }

    #[test]
    fn test_nonce_uniqueness() {
        let test_dir = TestDir::new();
        let plaintext = b"Same plaintext, different nonce";

        let encrypted1 = encrypt(test_dir.path(), plaintext).unwrap();
        let encrypted2 = encrypt(test_dir.path(), plaintext).unwrap();

        // Ciphertexts must differ (nonces are random)
        assert_ne!(encrypted1, encrypted2);

        // Both must decrypt to the same plaintext
        assert_eq!(decrypt(test_dir.path(), &encrypted1).unwrap(), plaintext);
        assert_eq!(decrypt(test_dir.path(), &encrypted2).unwrap(), plaintext);
    }

    // --- Encryptor struct tests ---

    #[test]
    fn test_encryptor_round_trip() {
        let test_dir = TestDir::new();
        let encryptor = Encryptor::new(test_dir.path()).unwrap();
        let plaintext = b"Encryptor-based encryption";

        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encryptor_nonce_uniqueness() {
        let test_dir = TestDir::new();
        let encryptor = Encryptor::new(test_dir.path()).unwrap();
        let plaintext = b"Nonce test";

        let enc1 = encryptor.encrypt(plaintext).unwrap();
        let enc2 = encryptor.encrypt(plaintext).unwrap();

        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_encryptor_decrypt_deterministic() {
        let test_dir = TestDir::new();
        let encryptor = Encryptor::new(test_dir.path()).unwrap();
        let plaintext = b"Deterministic decrypt test";

        let ciphertext = encryptor.encrypt(plaintext).unwrap();

        // Same ciphertext + same key must produce same plaintext every time
        for _ in 0..5 {
            assert_eq!(encryptor.decrypt(&ciphertext).unwrap(), plaintext);
        }
    }

    #[test]
    fn test_encryptor_different_key_fails() {
        let dir_a = TestDir::new();
        let dir_b = TestDir::new();

        let enc_a = Encryptor::new(dir_a.path()).unwrap();
        let enc_b = Encryptor::new(dir_b.path()).unwrap();

        let ciphertext = enc_a.encrypt(b"Secret").unwrap();
        let result = enc_b.decrypt(&ciphertext);

        assert!(matches!(result, Err(EncryptionError::DecryptionFailed)));
    }

    // --- Seed file tests ---

    #[test]
    fn test_seed_file_creation() {
        let test_dir = TestDir::new();
        let seed_path = test_dir.path().join(SEED_FILE_NAME);

        // Seed file should not exist initially
        assert!(!seed_path.exists());

        // Get or create key should create the seed file
        let key1 = get_or_create_key(test_dir.path()).unwrap();

        // Seed file should now exist
        assert!(seed_path.exists());

        // Key should be the correct length
        assert_eq!(key1.len(), KEY_LEN);
    }

    #[test]
    fn test_seed_file_persistence() {
        let test_dir = TestDir::new();
        let seed_path = test_dir.path().join(SEED_FILE_NAME);

        // Create key first time
        let key1 = get_or_create_key(test_dir.path()).unwrap();

        // Get key second time should return the same key
        let key2 = get_or_create_key(test_dir.path()).unwrap();

        assert_eq!(key1, key2);

        // Seed file should still exist
        assert!(seed_path.exists());
    }

    #[test]
    fn test_seed_file_unix_permissions() {
        let test_dir = TestDir::new();

        // Create key
        let _key = get_or_create_key(test_dir.path()).unwrap();

        // Check permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let seed_path = test_dir.path().join(SEED_FILE_NAME);
            let metadata = fs::metadata(&seed_path).unwrap();
            let mode = metadata.permissions().mode();
            // Check that file is readable/writable by owner only (0o600)
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn test_seed_file_corrupted_returns_error() {
        let test_dir = TestDir::new();
        let seed_path = test_dir.path().join(SEED_FILE_NAME);

        // Create a corrupted seed file (wrong length)
        fs::create_dir_all(test_dir.path()).unwrap();
        fs::write(&seed_path, vec![0u8; 10]).unwrap();

        // Should now return SeedCorrupted instead of silently regenerating
        let result = get_or_create_key(test_dir.path());
        assert!(matches!(result, Err(EncryptionError::SeedCorrupted)));
    }
}
