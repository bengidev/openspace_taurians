"use client";

import { useEffect } from "react";
import { useThemeStore, type ThemeMode } from "@/stores/themeStore";

// ── Monaco theme definitions ───────────────────────────────────────
// These token maps are derived from the same CSS variable palette
// used in globals.css, keeping editor and chrome visually consistent.
// They are consumed by monaco.editor.defineTheme() when the editor
// mounts and by the useMonacoTheme hook on theme changes.

/** Theme tokens for Monaco dark mode. */
export const monacoDarkTheme = {
  base: "vs-dark" as const,
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#18181b",
    "editor.foreground": "#fafafa",
    "editor.lineHighlightBackground": "#27272a",
    "editor.selectionBackground": "#3b82f640",
    "editorCursor.foreground": "#3b82f6",
    "editorWhitespace.foreground": "#71717a",
    "editorIndentGuide.background": "#3f3f46",
    "editorLineNumber.foreground": "#71717a",
    "editorLineNumber.activeForeground": "#fafafa",
    "scrollbar.shadow": "#000000",
    "scrollbarSlider.background": "#71717a60",
    "scrollbarSlider.hoverBackground": "#71717a80",
    "scrollbarSlider.activeBackground": "#71717aa0",
  },
};

/** Theme tokens for Monaco light mode. */
export const monacoLightTheme = {
  base: "vs" as const,
  inherit: true,
  rules: [],
  colors: {
    "editor.background": "#ffffff",
    "editor.foreground": "#171717",
    "editor.lineHighlightBackground": "#f5f5f5",
    "editor.selectionBackground": "#3b82f640",
    "editorCursor.foreground": "#3b82f6",
    "editorWhitespace.foreground": "#a3a3a3",
    "editorIndentGuide.background": "#e5e5e5",
    "editorLineNumber.foreground": "#a3a3a3",
    "editorLineNumber.activeForeground": "#171717",
    "scrollbar.shadow": "#00000020",
    "scrollbarSlider.background": "#a3a3a360",
    "scrollbarSlider.hoverBackground": "#a3a3a380",
    "scrollbarSlider.activeBackground": "#a3a3a3a0",
  },
};

const THEME_NAMES: Record<ThemeMode, string> = {
  dark: "openspace-dark",
  light: "openspace-light",
};

// Shared type for Monaco theme definitions.
interface MonacoThemeDef {
  base: "vs-dark" | "vs";
  inherit: boolean;
  rules: never[];
  colors: Record<string, string>;
}

const THEME_DEFS: Record<ThemeMode, MonacoThemeDef> = {
  dark: monacoDarkTheme,
  light: monacoLightTheme,
};

/**
 * Hook to apply Monaco editor theme.
 *
 * Call this from any component that creates a Monaco editor instance.
 * When Monaco is not yet loaded, the hook is a no-op but still
 * subscribes to theme changes so the theme is applied as soon as
 * the editor becomes available.
 *
 * @param editorRef - A ref to the Monaco editor instance, or null.
 */
export function useMonacoTheme(
  editorRef: React.MutableRefObject<unknown>,
): void {
  const mode = useThemeStore((s) => s.mode);

  useEffect(() => {
    // Only apply if Monaco is loaded and an editor instance exists.
    const monaco = (globalThis as Record<string, unknown>).monaco as
      | Record<string, unknown>
      | undefined;
    if (!monaco || !editorRef.current) return;

    const defineTheme = monaco.editor?.defineTheme as
      | ((name: string, def: unknown) => void)
      | undefined;
    const setTheme = monaco.editor?.setTheme as
      | ((name: string) => void)
      | undefined;

    if (defineTheme && setTheme) {
      defineTheme(THEME_NAMES[mode], THEME_DEFS[mode]);
      setTheme(THEME_NAMES[mode]);
    }
  }, [mode, editorRef]);
}
