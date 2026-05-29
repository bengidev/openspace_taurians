import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the providers API module used by inlineCompletionStore.
vi.mock("@/lib/api/providers", () => ({
  inlineCompletion: vi.fn(),
  activeProviderGet: vi.fn(),
}));

import { useInlineCompletionStore } from "@/stores/inlineCompletionStore";
import { inlineCompletion, activeProviderGet } from "@/lib/api/providers";
import type { InlineCompletionResponse } from "@/lib/types/provider";

describe("inlineCompletionStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset Zustand store state between tests.
    useInlineCompletionStore.setState({
      lastCompletion: null,
      isLoading: false,
      error: null,
      activeProvider: null,
      providerLoaded: false,
    });
  });

  // ── loadActiveProvider ──────────────────────────────────────

  it("loadActiveProvider fetches and stores the active provider", async () => {
    vi.mocked(activeProviderGet).mockResolvedValue({
      provider_id: 1,
      model: "gpt-4o",
    });

    await useInlineCompletionStore.getState().loadActiveProvider();

    const state = useInlineCompletionStore.getState();
    expect(state.activeProvider).toEqual({ provider_id: 1, model: "gpt-4o" });
    expect(state.providerLoaded).toBe(true);
    expect(state.error).toBeNull();
  });

  it("loadActiveProvider stores null when no active provider exists", async () => {
    vi.mocked(activeProviderGet).mockResolvedValue(null);

    await useInlineCompletionStore.getState().loadActiveProvider();

    const state = useInlineCompletionStore.getState();
    expect(state.activeProvider).toBeNull();
    expect(state.providerLoaded).toBe(true);
  });

  it("loadActiveProvider stores error on failure", async () => {
    vi.mocked(activeProviderGet).mockRejectedValue(
      new Error("database locked"),
    );

    await useInlineCompletionStore.getState().loadActiveProvider();

    const state = useInlineCompletionStore.getState();
    expect(state.error).toBe("database locked");
    expect(state.providerLoaded).toBe(true);
  });

  // ── getCompletion ───────────────────────────────────────────

  it("getCompletion sets error when no active provider", async () => {
    useInlineCompletionStore.setState({
      activeProvider: null,
      providerLoaded: true,
    });

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    const state = useInlineCompletionStore.getState();
    expect(state.error).toContain("No active provider");
    expect(state.lastCompletion).toBeNull();
    expect(state.isLoading).toBe(false);
  });

  it("getCompletion fetches and stores the completion result", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });

    const mockResponse: InlineCompletionResponse = {
      completion: "let x = 42;",
      provider_name: "OpenAI",
      model: "gpt-4o",
    };
    vi.mocked(inlineCompletion).mockResolvedValue(mockResponse);

    await useInlineCompletionStore
      .getState()
      .getCompletion("fn main() {\n    let x = ", 23);

    const state = useInlineCompletionStore.getState();
    expect(state.lastCompletion).toEqual(mockResponse);
    expect(state.isLoading).toBe(false);
    expect(state.error).toBeNull();
    expect(inlineCompletion).toHaveBeenCalledWith({
      document: "fn main() {\n    let x = ",
      cursor_position: 23,
    });
  });

  it("getCompletion stores error on provider failure", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });

    vi.mocked(inlineCompletion).mockRejectedValue(
      new Error("response path 'choices[0].message.content' did not resolve to a string"),
    );

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    const state = useInlineCompletionStore.getState();
    expect(state.error).toContain("response path");
    expect(state.lastCompletion).toBeNull();
    expect(state.isLoading).toBe(false);
  });

  it("getCompletion ignores duplicate calls while loading", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
      isLoading: true,
    });

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    expect(inlineCompletion).not.toHaveBeenCalled();
  });

  it("getCompletion clears previous completion before new request", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
      lastCompletion: {
        completion: "old",
        provider_name: "OpenAI",
        model: "gpt-4o",
      },
    });

    vi.mocked(inlineCompletion).mockResolvedValue({
      completion: "new",
      provider_name: "OpenAI",
      model: "gpt-4o",
    });

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    const state = useInlineCompletionStore.getState();
    expect(state.lastCompletion?.completion).toBe("new");
  });

  it("getCompletion sets loading state during request", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });

    let resolveCompletion!: (value: InlineCompletionResponse) => void;
    vi.mocked(inlineCompletion).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveCompletion = resolve;
        }),
    );

    const completionPromise = useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    // While in-flight, isLoading should be true.
    expect(useInlineCompletionStore.getState().isLoading).toBe(true);

    resolveCompletion({
      completion: "done",
      provider_name: "OpenAI",
      model: "gpt-4o",
    });
    await completionPromise;

    expect(useInlineCompletionStore.getState().isLoading).toBe(false);
  });

  // ── clearError ──────────────────────────────────────────────

  it("clearError removes the current error", () => {
    useInlineCompletionStore.setState({ error: "something went wrong" });

    useInlineCompletionStore.getState().clearError();

    expect(useInlineCompletionStore.getState().error).toBeNull();
  });

  // ── Provider error handling (AC: clear setup message) ────────

  it("getCompletion surfaces auth failure as clear error message", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 1, model: "gpt-4o" },
      providerLoaded: true,
    });

    vi.mocked(inlineCompletion).mockRejectedValue(
      new Error("provider returned HTTP 401: Unauthorized"),
    );

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    const state = useInlineCompletionStore.getState();
    expect(state.error).toContain("401");
  });

  it("getCompletion surfaces deleted provider as clear error message", async () => {
    useInlineCompletionStore.setState({
      activeProvider: { provider_id: 99, model: "gpt-4o" },
      providerLoaded: true,
    });

    vi.mocked(inlineCompletion).mockRejectedValue(
      new Error(
        "Active provider '99' not found. It may have been deleted. Open Settings → Providers to select a new one.",
      ),
    );

    await useInlineCompletionStore
      .getState()
      .getCompletion("code", 0);

    const state = useInlineCompletionStore.getState();
    expect(state.error).toContain("deleted");
    expect(state.error).toContain("Settings");
  });
});
