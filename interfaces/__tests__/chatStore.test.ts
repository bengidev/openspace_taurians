import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the providers API module used by chatStore.
vi.mock("@/lib/api/providers", () => ({
  chatSend: vi.fn(),
  activeProviderGet: vi.fn(),
}));

import { useChatStore } from "@/stores/chatStore";
import { chatSend, activeProviderGet } from "@/lib/api/providers";

function mockChatSend(tokens: string[]) {
  vi.mocked(chatSend).mockImplementation(async function* () {
    for (const token of tokens) {
      yield token;
    }
  });
}

function mockChatSendError(error: Error) {
  vi.mocked(chatSend).mockImplementation(async function* () {
    throw error;
  });
}

describe("chatStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset Zustand store state between tests.
    useChatStore.setState({
      messages: [],
      isLoading: false,
      error: null,
      activeProvider: null,
      providerLoaded: false,
    });
  });

  it("loadActiveProvider fetches and stores the active provider", async () => {
    vi.mocked(activeProviderGet).mockResolvedValue({
      provider_id: 1,
      model: "gpt-4o",
    });

    await useChatStore.getState().loadActiveProvider();

    const state = useChatStore.getState();
    expect(state.activeProvider).toEqual({ provider_id: 1, model: "gpt-4o" });
    expect(state.providerLoaded).toBe(true);
    expect(state.error).toBeNull();
  });

  it("loadActiveProvider stores null when no active provider exists", async () => {
    vi.mocked(activeProviderGet).mockResolvedValue(null);

    await useChatStore.getState().loadActiveProvider();

    const state = useChatStore.getState();
    expect(state.activeProvider).toBeNull();
    expect(state.providerLoaded).toBe(true);
  });

  it("sendMessage sets error when no active provider", async () => {
    useChatStore.setState({ activeProvider: null, providerLoaded: true });

    await useChatStore.getState().sendMessage("hello");

    const state = useChatStore.getState();
    expect(state.error).toContain("No active provider");
    expect(state.messages).toHaveLength(0);
  });

  it("sendMessage streams tokens into the assistant message", async () => {
    useChatStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });
    mockChatSend(["Hello", " world", "!"]);

    await useChatStore.getState().sendMessage("Say hello");

    const state = useChatStore.getState();
    expect(state.messages).toHaveLength(2);
    expect(state.messages[0]).toEqual({ role: "user", content: "Say hello" });
    expect(state.messages[1]).toEqual({
      role: "assistant",
      content: "Hello world!",
    });
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
  });

  it("sendMessage includes conversation history", async () => {
    useChatStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
      messages: [
        { role: "user", content: "first" },
        { role: "assistant", content: "reply" },
      ],
    });
    mockChatSend(["second reply"]);

    await useChatStore.getState().sendMessage("second");

    expect(chatSend).toHaveBeenCalledWith({
      messages: [
        { role: "user", content: "first" },
        { role: "assistant", content: "reply" },
        { role: "user", content: "second" },
      ],
    });
  });

  it("sendMessage removes empty assistant placeholder on error", async () => {
    useChatStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });
    mockChatSendError(new Error("response path did not resolve"));

    await useChatStore.getState().sendMessage("hello");

    const state = useChatStore.getState();
    // The empty assistant placeholder should be removed on failure.
    expect(state.messages).toHaveLength(1);
    expect(state.messages[0]).toEqual({ role: "user", content: "hello" });
    expect(state.error).toContain("response path");
    expect(state.isLoading).toBe(false);
  });

  it("clearMessages resets messages and error", () => {
    useChatStore.setState({
      messages: [
        { role: "user", content: "hi" },
        { role: "assistant", content: "hello" },
      ],
      error: "some error",
    });

    useChatStore.getState().clearMessages();

    const state = useChatStore.getState();
    expect(state.messages).toHaveLength(0);
    expect(state.error).toBeNull();
  });

  it("clearError removes the current error", () => {
    useChatStore.setState({ error: "something went wrong" });

    useChatStore.getState().clearError();

    expect(useChatStore.getState().error).toBeNull();
  });

  it("sendMessage ignores duplicate calls while loading", async () => {
    useChatStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
      isLoading: true,
    });

    await useChatStore.getState().sendMessage("should be ignored");

    expect(chatSend).not.toHaveBeenCalled();
  });
});