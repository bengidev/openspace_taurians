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

// Send and Sync are auto-derived from tauri::ipc::Channel<T>.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_channel_creation_from_tauri() {
        let tauri_channel = tauri::ipc::Channel::<String>::new(|_| Ok(()));
        let channel = Channel::from_tauri(tauri_channel);
        // Channel ID is assigned by Tauri; just verify it's accessible
        let _id = channel.id();
    }

    #[test]
    fn test_channel_send() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let tauri_channel = tauri::ipc::Channel::<String>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let s: String = serde_json::from_str(&s).expect("malformed JSON in test channel callback");
                received_clone.lock().unwrap().push(s);
            }
            Ok(())
        });

        let channel = Channel::from_tauri(tauri_channel);
        channel.send("hello".to_string()).unwrap();
        channel.send("world".to_string()).unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0], "hello");
        assert_eq!(received[1], "world");
    }

    #[test]
    fn test_channel_clone() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let tauri_channel = tauri::ipc::Channel::<i32>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let n: i32 = serde_json::from_str(&s).expect("malformed JSON in test channel callback");
                received_clone.lock().unwrap().push(n);
            }
            Ok(())
        });

        let channel = Channel::from_tauri(tauri_channel);
        let channel_clone = channel.clone();

        channel.send(1).unwrap();
        channel_clone.send(2).unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0], 1);
        assert_eq!(received[1], 2);
    }

    #[test]
    fn test_channel_into_inner() {
        let tauri_channel = tauri::ipc::Channel::<String>::new(|_| Ok(()));
        let channel = Channel::from_tauri(tauri_channel);
        let _inner = channel.into_inner();
    }

    #[test]
    fn test_channel_error_display() {
        let err = ChannelError::SendFailed("test error".into());
        assert_eq!(
            err.to_string(),
            "failed to send message through channel: test error"
        );
    }
}
