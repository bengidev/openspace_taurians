//! Stream utilities — reusable streaming helpers for Tauri applications.
//!
//! This crate provides a [`Channel`] wrapper around `tauri::ipc::Channel<T>`
//! and an [`into_channel_stream`] helper that converts any `futures::Stream`
//! into a Tauri Channel stream.
//!
//! # Example
//!
//! ```rust,ignore
//! use stream_utils::{Channel, into_channel_stream};
//! use futures::stream;
//!
//! #[tauri::command]
//! async fn my_streaming_command(on_event: tauri::ipc::Channel<String>) {
//!     let stream = stream::iter(vec!["hello".to_string(), "world".to_string()]);
//!     let channel = Channel::from_tauri(on_event);
//!     into_channel_stream(stream, &channel).await.unwrap();
//! }
//! ```

mod channel;
mod stream;

pub use channel::{Channel, ChannelError};
pub use stream::into_channel_stream;
