/**
 * Stream utilities for Tauri IPC
 *
 * Provides helpers for consuming streaming data from Tauri commands
 * that use `tauri::ipc::Channel<T>`.
 */

import { invoke } from "@tauri-apps/api/core";
import { Channel } from "@tauri-apps/api/core";

/**
 * Options for invokeStream
 */
export interface InvokeStreamOptions {
  /**
   * The parameter name to pass the channel as.
   * Defaults to "onEvent".
   */
  channelParam?: string;
}

/**
 * Invoke a Tauri command that streams data back via a Channel.
 *
 * This function creates a Channel, passes it to the Tauri command,
 * and returns an AsyncIterable that yields each item as it arrives.
 *
 * @param command - The Tauri command name to invoke
 * @param args - Arguments to pass to the command (excluding the channel)
 * @param options - Optional configuration
 * @returns An AsyncIterable that yields items from the stream
 *
 * @example
 * ```typescript
 * // Rust side:
 * // #[tauri::command]
 * // async fn stream_numbers(on_event: Channel<i32>) {
 * //   for i in 0..5 {
 * //     on_event.send(i).unwrap();
 * //   }
 * // }
 *
 * // TypeScript side:
 * for await (const num of invokeStream<number>("stream_numbers")) {
 *   console.log(num); // 0, 1, 2, 3, 4
 * }
 * ```
 */
export async function* invokeStream<T>(
  command: string,
  args?: Record<string, unknown>,
  options?: InvokeStreamOptions
): AsyncIterable<T> {
  const channelParam = options?.channelParam ?? "onEvent";

  // Buffer to hold items that arrive before the consumer calls next()
  const buffer: T[] = [];
  // Resolve function for the current waiter, if any
  let waiter: ((value: IteratorResult<T>) => void) | null = null;
  // Whether the stream has completed
  let done = false;
  // Error that occurred during streaming, if any
  let error: Error | null = null;

  // Create the channel that will receive items from the Rust side
  const channel = new Channel<T>();
  channel.onmessage = (message: T) => {
    if (waiter) {
      // Consumer is waiting, deliver directly
      const resolve = waiter;
      waiter = null;
      resolve({ value: message, done: false });
    } else {
      // No waiter, buffer the item
      buffer.push(message);
    }
  };

  // Start the invoke call (don't await it yet)
  const invokePromise = invoke(command, {
    ...args,
    [channelParam]: channel,
  })
    .then(() => {
      // Command completed successfully
      done = true;
      if (waiter) {
        const resolve = waiter;
        waiter = null;
        resolve({ value: undefined, done: true });
      }
    })
    .catch((err) => {
      // Command failed
      done = true;
      error = err instanceof Error ? err : new Error(String(err));
      if (waiter) {
        const resolve = waiter;
        waiter = null;
        resolve({ value: undefined, done: true });
      }
    });

  try {
    // Yield items as they arrive
    while (true) {
      if (buffer.length > 0) {
        // Item already buffered
        yield buffer.shift()!;
      } else if (done) {
        // Stream completed
        if (error) {
          throw error;
        }
        return;
      } else {
        // Wait for next item or completion
        const result = await new Promise<IteratorResult<T>>((resolve) => {
          waiter = resolve;
        });
        if (result.done) {
          if (error) {
            throw error;
          }
          return;
        }
        yield result.value;
      }
    }
  } finally {
    // Ensure the invoke promise settles to avoid unhandled rejections
    await invokePromise.catch(() => {});
  }
}
