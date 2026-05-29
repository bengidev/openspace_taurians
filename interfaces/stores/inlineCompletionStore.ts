"use client";

import { create } from "zustand";
import { inlineCompletion, activeProviderGet } from "@/lib/api/providers";
import type { ActiveProvider, InlineCompletionResponse } from "@/lib/types/provider";

// ── Types ──────────────────────────────────────────────────────────

export interface InlineCompletionState {
  /** The last completion result from the provider. */
  lastCompletion: InlineCompletionResponse | null;
  /** Whether a completion request is in flight. */
  isLoading: boolean;
  /** Error message, if any. */
  error: string | null;
  /** The currently active provider/model, if configured. */
  activeProvider: ActiveProvider | null;
  /** Whether the active provider has been loaded at least once. */
  providerLoaded: boolean;

  // Actions
  loadActiveProvider: () => Promise<void>;
  getCompletion: (document: string, cursorPosition: number) => Promise<void>;
  clearError: () => void;
}

// ── Store ──────────────────────────────────────────────────────────

export const useInlineCompletionStore = create<InlineCompletionState>()(
  (set, get) => ({
    lastCompletion: null,
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

    getCompletion: async (document: string, cursorPosition: number) => {
      const state = get();
      if (state.isLoading) return;

      if (!state.activeProvider) {
        set({
          error:
            "No active provider configured. Open Settings → Providers to choose a provider and model.",
        });
        return;
      }

      set({ isLoading: true, error: null, lastCompletion: null });

      try {
        const response = await inlineCompletion({
          document,
          cursor_position: cursorPosition,
        });
        set({ lastCompletion: response, isLoading: false });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        set({ error: message, isLoading: false });
      }
    },

    clearError: () => set({ error: null }),
  }),
);
