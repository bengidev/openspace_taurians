"use client";

import { create } from "zustand";
import { chatSend, chatCancel, activeProviderGet } from "@/lib/api/providers";
import type { ActiveProvider, ChatMessage } from "@/lib/types/provider";

// ── Types ──────────────────────────────────────────────────────────

export interface ChatMessageState {
  role: "user" | "assistant";
  content: string;
}

export interface ChatState {
  messages: ChatMessageState[];
  isLoading: boolean;
  error: string | null;
  activeProvider: ActiveProvider | null;
  providerLoaded: boolean;
  abortController: AbortController | null;

  // Actions
  loadActiveProvider: () => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
  cancelStream: () => void;
  clearMessages: () => void;
  clearError: () => void;
}

// ── Store ──────────────────────────────────────────────────────────

export const useChatStore = create<ChatState>()((set, get) => ({
  messages: [],
  isLoading: false,
  error: null,
  activeProvider: null,
  providerLoaded: false,
  abortController: null,

  loadActiveProvider: async () => {
    try {
      const active = await activeProviderGet();
      set({ activeProvider: active, providerLoaded: true, error: null });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      set({ error: message, providerLoaded: true });
    }
  },

  sendMessage: async (content: string) => {
    const state = get();
    if (state.isLoading) return;

    if (!state.activeProvider) {
      set({ error: "No active provider configured. Open Settings → Providers to choose a provider and model." });
      return;
    }

    // Cancel any previous stream still in flight.
    state.abortController?.abort();

    const abortController = new AbortController();

    const userMessage: ChatMessageState = { role: "user", content };
    const assistantMessage: ChatMessageState = { role: "assistant", content: "" };

    set((s) => ({
      messages: [...s.messages, userMessage, assistantMessage],
      isLoading: true,
      error: null,
      abortController,
    }));

    try {
      // Build the full conversation history for the provider.
      const history: ChatMessage[] = [
        ...state.messages.map((m) => ({ role: m.role, content: m.content })),
        { role: "user", content },
      ];

      for await (const token of chatSend({
        messages: history,
        signal: abortController.signal,
      })) {
        set((s) => {
          const messages = [...s.messages];
          const last = messages[messages.length - 1];
          if (last && last.role === "assistant") {
            messages[messages.length - 1] = {
              ...last,
              content: last.content + token,
            };
          }
          return { messages };
        });
      }
    } catch (err) {
      // If the stream was aborted, silently remove the empty placeholder
      // and don't surface an error — the user initiated the cancellation.
      if (abortController.signal.aborted) {
        set((s) => {
          const messages = [...s.messages];
          const last = messages[messages.length - 1];
          if (last && last.role === "assistant" && last.content === "") {
            messages.pop();
          }
          return { messages };
        });
      } else {
        const message = err instanceof Error ? err.message : String(err);
        set((s) => {
          const messages = [...s.messages];
          const last = messages[messages.length - 1];
          if (last && last.role === "assistant" && last.content === "") {
            // Remove the empty assistant placeholder on failure.
            messages.pop();
          }
          return { messages, error: message };
        });
      }
    } finally {
      // Only clear loading state if this is still the active controller.
      // A subsequent sendMessage may have replaced it.
      if (get().abortController === abortController) {
        set({ isLoading: false, abortController: null });
      }
    }
  },

  cancelStream: () => {
    const { abortController } = get();
    if (abortController) {
      abortController.abort();
      // Also cancel on the Rust side so the HTTP connection is dropped
      // immediately instead of waiting for the channel send to fail.
      chatCancel().catch(() => {});
    }
  },

  clearMessages: () => {
    // Cancel any active stream before clearing.
    const { abortController } = get();
    if (abortController) {
      abortController.abort();
      chatCancel().catch(() => {});
    }
    set({ messages: [], error: null, isLoading: false, abortController: null });
  },

  clearError: () => set({ error: null }),
}));