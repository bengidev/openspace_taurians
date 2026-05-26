"use client";

import { create } from "zustand";
import { persist } from "zustand/middleware";

// ── Types ──────────────────────────────────────────────────────────

export type ThemeMode = "dark" | "light";

export interface ThemeState {
  mode: ThemeMode;
  accentColor: string;

  // Actions
  toggle: () => void;
  setMode: (mode: ThemeMode) => void;
  setAccent: (color: string) => void;
}

// ── CSS variable application ───────────────────────────────────────

/** Apply theme CSS variables to :root based on the current mode. */
function applyThemeToDocument(mode: ThemeMode, accentColor: string): void {
  const root = document.documentElement;

  // Toggle the .dark class on <html> so Tailwind's darkMode: "class" works.
  if (mode === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }

  // Update the accent color CSS variable.
  root.style.setProperty("--color-accent", accentColor);
}

// ── Store ──────────────────────────────────────────────────────────

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      mode: "dark",
      accentColor: "#3b82f6",

      toggle: () => {
        const nextMode = get().mode === "dark" ? "light" : "dark";
        set({ mode: nextMode });
        applyThemeToDocument(nextMode, get().accentColor);
      },

      setMode: (mode: ThemeMode) => {
        set({ mode });
        applyThemeToDocument(mode, get().accentColor);
      },

      setAccent: (color: string) => {
        set({ accentColor: color });
        applyThemeToDocument(get().mode, color);
      },
    }),
    {
      name: "openspace-theme",
      // Only persist mode and accentColor (functions are excluded automatically).
      partialize: (state) => ({
        mode: state.mode,
        accentColor: state.accentColor,
      }),
      onRehydrateStorage: () => (state) => {
        // When the store is rehydrated from localStorage, apply the theme
        // to the document immediately so the UI reflects the saved choice.
        if (state) {
          applyThemeToDocument(state.mode, state.accentColor);
        }
      },
    },
  ),
);
