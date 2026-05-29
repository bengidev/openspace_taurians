import { invoke } from "@tauri-apps/api/core";
import type { ActiveProvider, ChatMessage, InlineCompletionRequest, InlineCompletionResponse, Provider, ProviderCreate, ProviderTestResult, ProviderUpdate } from "../types/provider";
import { invokeStream } from "../stream";

export async function providerList(): Promise<Provider[]> {
  return invoke("provider_list");
}

export async function providerGet(id: number): Promise<Provider | null> {
  return invoke("provider_get", { id });
}

export async function providerCreate(config: ProviderCreate): Promise<number> {
  return invoke("provider_create", { ...config });
}

export async function providerUpdate(config: ProviderUpdate): Promise<boolean> {
  return invoke("provider_update", { ...config });
}

export async function providerDelete(id: number): Promise<boolean> {
  return invoke("provider_delete", { id });
}

export async function providerTestConnection(providerId: number): Promise<ProviderTestResult> {
  return invoke("provider_test_connection", { providerId });
}

export async function activeProviderGet(): Promise<ActiveProvider | null> {
  return invoke("active_provider_get");
}

export async function activeProviderSet(providerId: number, model: string): Promise<void> {
  return invoke("active_provider_set", { providerId, model });
}

export async function activeProviderClear(): Promise<boolean> {
  return invoke("active_provider_clear");
}

// ── Chat API ────────────────────────────────────────────────────

export interface ChatSendOptions {
  messages: ChatMessage[];
  temperature?: number;
  /**
   * Optional AbortSignal to cancel the stream from the JS side.
   * For full cancellation (stopping the Rust-side HTTP read), also call
   * {@link chatCancel}.
   */
  signal?: AbortSignal;
}

/**
 * Stream a chat completion through the active provider/model.
 *
 * Yields string tokens as the provider produces them. The chat UI
 * does not need to know which provider is behind the adapter.
 *
 * @throws if no active provider is configured or if the provider
 *         returns an error (bad response path, auth failure, etc.).
 * @throws {DOMException} with name "AbortError" if the signal is aborted.
 */
export async function* chatSend(
  options: ChatSendOptions,
): AsyncIterable<string> {
  yield* invokeStream<string>(
    "chat_send_stream",
    {
      messages: options.messages,
      temperature: options.temperature ?? 0.7,
    },
    { channelParam: "onToken", signal: options.signal },
  );
}

/**
 * Cancel the currently active chat stream, if any.
 *
 * Returns `true` if a stream was cancelled, `false` if no stream was
 * active. This fires the Rust-side cancellation token, which drops the
 * stream future and closes the HTTP connection immediately.
 */
export async function chatCancel(): Promise<boolean> {
  return invoke("chat_cancel");
}

// ── Inline completion API ────────────────────────────────────────

/**
 * Request an inline code completion through the active provider/model.
 *
 * Sends the document context through the same generic provider adapter
 * used by chat. The caller does not need to know which provider is
 * behind the adapter — it simply passes document context and receives
 * a completion string.
 *
 * @throws if no active provider is configured or if the provider
 *         returns an error (bad response path, auth failure, etc.).
 */
export async function inlineCompletion(
  request: InlineCompletionRequest,
): Promise<InlineCompletionResponse> {
  return invoke("inline_completion", { request });
}
