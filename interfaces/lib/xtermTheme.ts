"use client";

import { useEffect } from "react";
import { useThemeStore, type ThemeMode } from "@/stores/themeStore";

// ── xterm.js theme definitions ─────────────────────────────────────
// Maps to ITerminalOptions.theme. Derived from the same CSS variable
// palette as globals.css and monacoTheme.ts for visual consistency.

/** xterm.js theme object for dark mode. */
export const xtermDarkTheme = {
  background: "#18181b",
  foreground: "#fafafa",
  cursor: "#3b82f6",
  cursorAccent: "#18181b",
  selectionBackground: "#3b82f660",
  selectionForeground: "#fafafa",
  selectionInactiveBackground: "#71717a40",
  black: "#27272a",
  red: "#f87171",
  green: "#4ade80",
  yellow: "#fbbf24",
  blue: "#60a5fa",
  magenta: "#c084fc",
  cyan: "#22d3ee",
  white: "#fafafa",
  brightBlack: "#3f3f46",
  brightRed: "#fca5a5",
  brightGreen: "#86efac",
  brightYellow: "#fde047",
  brightBlue: "#93c5fd",
  brightMagenta: "#d8b4fe",
  brightCyan: "#67e8f9",
  brightWhite: "#ffffff",
};

/** xterm.js theme object for light mode. */
export const xtermLightTheme = {
  background: "#ffffff",
  foreground: "#171717",
  cursor: "#3b82f6",
  cursorAccent: "#ffffff",
  selectionBackground: "#3b82f640",
  selectionForeground: "#171717",
  selectionInactiveBackground: "#a3a3a330",
  black: "#171717",
  red: "#dc2626",
  green: "#16a34a",
  yellow: "#ca8a04",
  blue: "#2563eb",
  magenta: "#9333ea",
  cyan: "#0891b2",
  white: "#a3a3a3",
  brightBlack: "#525252",
  brightRed: "#ef4444",
  brightGreen: "#22c55e",
  brightYellow: "#eab308",
  brightBlue: "#3b82f6",
  brightMagenta: "#a855f7",
  brightCyan: "#06b6d4",
  brightWhite: "#d4d4d4",
};

const XTERM_THEMES: Record<ThemeMode, typeof xtermDarkTheme> = {
  dark: xtermDarkTheme,
  light: xtermLightTheme,
};

/**
 * Hook to apply xterm.js terminal theme.
 *
 * Call this from any component that creates an xterm.js Terminal
 * instance. When xterm.js is not yet loaded, the hook is a no-op
 * but still subscribes to theme changes.
 *
 * @param termRef - A ref to the xterm.js Terminal instance, or null.
 */
export function useXtermTheme(
  termRef: React.MutableRefObject<unknown>,
): void {
  const mode = useThemeStore((s) => s.mode);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;

    const optionsSetter = (term as Record<string, unknown>).options as
      | Record<string, unknown>
      | undefined;
    if (!optionsSetter) return;

    optionsSetter.theme = XTERM_THEMES[mode];
  }, [mode, termRef]);
}
