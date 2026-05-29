import { describe, it, expect, vi, beforeEach, Mock } from "vitest";
import { chatSend } from "../lib/api/providers";

interface MockChannelInstance {
  onmessage: ((message: unknown) => void) | null;
}

vi.mock("@tauri-apps/api/core", () => {
  class MockChannel {
    onmessage: ((message: unknown) => void) | null = null;
  }

  return {
    invoke: vi.fn(),
    Channel: MockChannel,
  };
});

describe("chatSend", () => {
  let mockInvoke: Mock;

  beforeEach(async () => {
    vi.clearAllMocks();
    const tauriCore = await import("@tauri-apps/api/core");
    mockInvoke = tauriCore.invoke as Mock;
  });

  it("streams tokens from the active provider through chat_send_stream", async () => {
    mockInvoke.mockImplementation(
      (_command: string, args: Record<string, unknown>) => {
        const channel = args.onToken as MockChannelInstance;
        setTimeout(() => channel.onmessage?.("Hello"), 10);
        setTimeout(() => channel.onmessage?.(" world"), 20);
        return new Promise((resolve) => setTimeout(resolve, 30));
      },
    );

    const tokens: string[] = [];
    for await (const token of chatSend({
      messages: [{ role: "user", content: "Say hello" }],
    })) {
      tokens.push(token);
    }

    expect(tokens).toEqual(["Hello", " world"]);
    expect(mockInvoke).toHaveBeenCalledWith("chat_send_stream", {
      messages: [{ role: "user", content: "Say hello" }],
      temperature: 0.7,
      onToken: expect.any(Object),
    });
  });

  it("passes custom temperature to chat_send_stream", async () => {
    mockInvoke.mockResolvedValue(undefined);

    const tokens: string[] = [];
    for await (const token of chatSend({
      messages: [{ role: "user", content: "test" }],
      temperature: 0.3,
    })) {
      tokens.push(token);
    }

    expect(mockInvoke).toHaveBeenCalledWith("chat_send_stream", {
      messages: [{ role: "user", content: "test" }],
      temperature: 0.3,
      onToken: expect.any(Object),
    });
  });

  it("propagates errors when no active provider is configured", async () => {
    mockInvoke.mockRejectedValue(
      new Error(
        "No active provider configured. Open Settings → Providers to choose a provider and model.",
      ),
    );

    const tokens: string[] = [];
    await expect(async () => {
      for await (const token of chatSend({
        messages: [{ role: "user", content: "hello" }],
      })) {
        tokens.push(token);
      }
    }).rejects.toThrow("No active provider configured");
  });

  it("propagates response-path extraction errors", async () => {
    mockInvoke.mockRejectedValue(
      new Error("response path 'choices[0].message.content' did not resolve to a string"),
    );

    const tokens: string[] = [];
    await expect(async () => {
      for await (const token of chatSend({
        messages: [{ role: "user", content: "hello" }],
      })) {
        tokens.push(token);
      }
    }).rejects.toThrow("response path");
  });

  it("handles empty stream (no tokens)", async () => {
    mockInvoke.mockResolvedValue(undefined);

    const tokens: string[] = [];
    for await (const token of chatSend({
      messages: [{ role: "user", content: "hello" }],
    })) {
      tokens.push(token);
    }

    expect(tokens).toEqual([]);
  });
});
