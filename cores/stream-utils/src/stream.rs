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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::time::timeout;

    fn make_test_channel<T>(
        received: Arc<Mutex<Vec<T>>>,
    ) -> Channel<T>
    where
        T: Serialize + serde::de::DeserializeOwned + Clone + Send + 'static,
    {
        let tauri_channel = tauri::ipc::Channel::<T>::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let item: T = serde_json::from_str(&s).expect("malformed JSON in test channel callback");
                received.lock().unwrap().push(item);
            }
            Ok(())
        });
        Channel::from_tauri(tauri_channel)
    }

    #[tokio::test]
    async fn test_into_channel_stream_basic() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let channel = make_test_channel(received.clone());

        let stream = stream::iter(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        into_channel_stream(stream, &channel).await.unwrap();

        let received = received.lock().unwrap();
        assert_eq!(*received, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[tokio::test]
    async fn test_into_channel_stream_empty() {
        let received = Arc::new(Mutex::new(Vec::<i32>::new()));
        let channel = make_test_channel(received.clone());

        let stream = stream::iter(Vec::<i32>::new());
        into_channel_stream(stream, &channel).await.unwrap();

        let received = received.lock().unwrap();
        assert!(received.is_empty());
    }

    #[tokio::test]
    async fn test_backpressure_natural_ordering() {
        // Verify items are sent one at a time in order (natural backpressure)
        let received = Arc::new(Mutex::new(Vec::new()));
        let channel = make_test_channel(received.clone());

        let items: Vec<i32> = (0..100).collect();
        let stream = stream::iter(items.clone());
        into_channel_stream(stream, &channel).await.unwrap();

        let received = received.lock().unwrap();
        assert_eq!(*received, items);
    }

    #[tokio::test]
    async fn test_backpressure_with_slow_channel() {
        // Simulate backpressure by using a channel callback that takes time
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let tauri_channel = tauri::ipc::Channel::<i32>::new(move |body| {
            // Simulate slow processing
            std::thread::sleep(Duration::from_millis(1));
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let n: i32 = serde_json::from_str(&s).expect("malformed JSON in test channel callback");
                received_clone.lock().unwrap().push(n);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let stream = stream::iter(vec![1, 2, 3, 4, 5]);
        into_channel_stream(stream, &channel).await.unwrap();

        let received = received.lock().unwrap();
        assert_eq!(*received, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_cancellation_drop_future() {
        // If the future is dropped, the stream stops producing items
        let received = Arc::new(Mutex::new(Vec::new()));
        let channel = make_test_channel(received.clone());

        // Create a slow stream that we'll cancel mid-way
        let stream = stream::iter(0..1000i32).then(|i| async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            i
        });
        // Box the stream to make it Unpin
        let stream = Box::pin(stream);

        let future = into_channel_stream(stream, &channel);

        // Give it a tiny bit of time then drop the future
        let result = timeout(Duration::from_millis(50), future).await;
        assert!(result.is_err(), "future should have been cancelled by timeout");

        // Some items may have been sent before cancellation
        let received = received.lock().unwrap();
        assert!(
            received.len() < 1000,
            "stream should have been cancelled, got {} items",
            received.len()
        );
    }

    #[tokio::test]
    async fn test_stream_exhaustion_stops_sending() {
        // When the stream is exhausted, the channel sees no more items
        let received = Arc::new(Mutex::new(Vec::new()));
        let channel = make_test_channel(received.clone());

        let stream = stream::iter(vec![1, 2, 3]);
        into_channel_stream(stream, &channel).await.unwrap();

        // Stream is exhausted, no more items should arrive
        let count = received.lock().unwrap().len();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_into_channel_stream_send_error_stops_stream() {
        // When the channel callback fails, the stream should stop and return an error
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let call_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let tauri_channel = tauri::ipc::Channel::<i32>::new(move |body| {
            call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let count = call_count_clone.load(std::sync::atomic::Ordering::SeqCst);
            if count == 2 {
                // Fail on the second item
                return Err(tauri::Error::WebviewNotFound);
            }
            if let tauri::ipc::InvokeResponseBody::Json(s) = body {
                let n: i32 = serde_json::from_str(&s).expect("malformed JSON in test channel callback");
                received_clone.lock().unwrap().push(n);
            }
            Ok(())
        });
        let channel = Channel::from_tauri(tauri_channel);

        let stream = stream::iter(vec![1, 2, 3]);
        let result = into_channel_stream(stream, &channel).await;

        assert!(result.is_err(), "expected send error, got success");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("failed to send message through channel"),
            "unexpected error message: {err_msg}"
        );

        // Only item 1 should have been received; item 2 failed and item 3 was never polled
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1, "only the first item should have been received");
        assert_eq!(received[0], 1);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_streams() {
        let received1 = Arc::new(Mutex::new(Vec::new()));
        let received2 = Arc::new(Mutex::new(Vec::new()));

        let channel1 = make_test_channel(received1.clone());
        let channel2 = make_test_channel(received2.clone());

        let stream1 = stream::iter(vec!["a1".to_string(), "a2".to_string(), "a3".to_string()]);
        let stream2 = stream::iter(vec!["b1".to_string(), "b2".to_string(), "b3".to_string()]);

        // Run both streams concurrently
        let (r1, r2) = tokio::join!(
            into_channel_stream(stream1, &channel1),
            into_channel_stream(stream2, &channel2),
        );

        r1.unwrap();
        r2.unwrap();

        let r1 = received1.lock().unwrap();
        let r2 = received2.lock().unwrap();
        assert_eq!(*r1, vec!["a1".to_string(), "a2".to_string(), "a3".to_string()]);
        assert_eq!(*r2, vec!["b1".to_string(), "b2".to_string(), "b3".to_string()]);
    }

    #[tokio::test]
    async fn test_concurrent_streams_shared_channel() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let channel = make_test_channel(received.clone());
        let channel2 = channel.clone();

        let stream1 = stream::iter(vec![1, 2, 3]);
        let stream2 = stream::iter(vec![10, 20, 30]);

        // Run both streams concurrently on the same channel
        let (r1, r2) = tokio::join!(
            into_channel_stream(stream1, &channel),
            into_channel_stream(stream2, &channel2),
        );

        r1.unwrap();
        r2.unwrap();

        let mut received = received.lock().unwrap().clone();
        received.sort();
        assert_eq!(received, vec![1, 2, 3, 10, 20, 30]);
    }
}
