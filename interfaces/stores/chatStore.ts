"use client";

import { create } from "zustand";
import { chatSend, activeProviderGet } from "@/lib/api/providers";
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

  // Actions
  loadActiveProvider: () => Promise<void>;
  sendMessage: (content: string) => Promise<void>;
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

    const userMessage: ChatMessageState = { role: "user", content };
    const assistantMessage: ChatMessageState = { role: "assistant", content: "" };

    set((s) => ({
      messages: [...s.messages, userMessage, assistantMessage],
      isLoading: true,
      error: null,
    }));

    try {
      // Build the full conversation history for the provider.
      const history: ChatMessage[] = [
        ...state.messages.map((m) => ({ role: m.role, content: m.content })),
        { role: "user", content },
      ];

      for await (const token of chatSend({ messages: history })) {
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
    } finally {
      set({ isLoading: false });
    }
  },

  clearMessages: () => set({ messages: [], error: null }),

  clearError: () => set({ error: null }),
}));