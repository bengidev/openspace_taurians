//! Stream-to-channel bridge — converts a `futures::Stream` into channel messages.

use futures::stream::Stream;
use futures::StreamExt;
use serde::Serialize;

use crate::channel::{Channel, ChannelError};

/// Drain a `futures::Stream` and forward each item through a [`Channel`].
///
/// This function consumes the stream item-by-item and sends each one through
/// the provided channel. It completes when the stream is exhausted or when
/// a send error occurs.
///
/// # Arguments
///
/// * `stream` - Any `futures::Stream` that yields items of type `T`
/// * `channel` - A [`Channel`] to send each item through
///
/// # Returns
///
/// `Ok(())` when the stream is fully consumed, or `Err(ChannelError)` if
/// a send fails (which typically means the receiver has disconnected).
///
/// # Cancellation
///
/// If the returned future is dropped before completion, the stream is also
/// dropped, effectively cancelling the operation.
///
/// # Backpressure
///
/// This function provides natural backpressure: it waits for each `send()`
/// to complete before polling the stream for the next item. If the channel
/// is slow to process messages, the stream will not advance.
pub async fn into_channel_stream<S, T>(
    mut stream: S,
    channel: &Channel<T>,
) -> Result<(), ChannelError>
where
    S: Stream<Item = T> + Unpin,
    T: Serialize + Clone + Send + 'static,
{
    while let Some(item) = stream.next().await {
        channel.send(item)?;
    }
    Ok(())
}
