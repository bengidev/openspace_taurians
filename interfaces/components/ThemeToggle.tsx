"use client";

import { useThemeStore } from "@/stores/themeStore";

/**
 * Theme toggle button for the workspace toolbar.
 * Shows a moon icon in light mode (click to go dark)
 * and a sun icon in dark mode (click to go light).
 */
export function ThemeToggle() {
  const mode = useThemeStore((s) => s.mode);
  const toggle = useThemeStore((s) => s.toggle);

  const isDark = mode === "dark";

  return (
    <button
      onClick={toggle}
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      title={isDark ? "Switch to light mode" : "Switch to dark mode"}
      className="flex items-center gap-1 px-2 py-0.5 text-xs rounded text-zinc-500 hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-700"
    >
      {isDark ? "☀️" : "🌙"}
    </button>
  );
}
