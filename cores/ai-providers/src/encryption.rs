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
