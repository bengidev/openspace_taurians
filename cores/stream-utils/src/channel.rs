//! Channel wrapper — provides a cleaner API around `tauri::ipc::Channel<T>`.

use serde::Serialize;
use thiserror::Error;

/// Errors that can occur when using [`Channel`].
#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("failed to send message through channel: {0}")]
    SendFailed(String),
}

/// A wrapper around `tauri::ipc::Channel<T>` that provides a cleaner API
/// and better error handling.
///
/// This type is `Clone`, `Send`, and `Sync` when `T` is.
pub struct Channel<T: Serialize + Clone + Send + 'static> {
    inner: tauri::ipc::Channel<T>,
}

impl<T: Serialize + Clone + Send + 'static> Channel<T> {
    /// Create a new `Channel` from a `tauri::ipc::Channel<T>`.
    pub fn from_tauri(channel: tauri::ipc::Channel<T>) -> Self {
        Self { inner: channel }
    }

    /// Send an item through the channel.
    ///
    /// # Errors
    ///
    /// Returns [`ChannelError::SendFailed`] if the underlying Tauri channel
    /// fails to send the message.
    pub fn send(&self, item: T) -> Result<(), ChannelError> {
        self.inner
            .send(item)
            .map_err(|e| ChannelError::SendFailed(e.to_string()))
    }

    /// Get the channel ID.
    pub fn id(&self) -> u32 {
        self.inner.id()
    }

    /// Consume this wrapper and return the underlying `tauri::ipc::Channel<T>`.
    pub fn into_inner(self) -> tauri::ipc::Channel<T> {
        self.inner
    }
}

impl<T: Serialize + Clone + Send + 'static> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// Safety: Channel is Send + Sync when T is Send + Sync
// tauri::ipc::Channel<T> is Send + Sync when T is Send + Sync
unsafe impl<T: Serialize + Clone + Send + 'static> Send for Channel<T> {}
unsafe impl<T: Serialize + Clone + Send + 'static> Sync for Channel<T> {}
