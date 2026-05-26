"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { CommandAction, ShortcutEntry, ShortcutScope } from "@/lib/shortcuts";

// ── Context shape ───────────────────────────────────────────────────

interface ShortcutContextValue {
  /** Register an application-level shortcut. */
  register: (entry: ShortcutEntry) => void;
  /** Unregister a previously registered shortcut by id. */
  unregister: (id: string) => void;
  /** All currently registered shortcuts (for command palette). */
  shortcuts: ReadonlyMap<string, ShortcutEntry>;
  /** Open/close the command palette. */
  setPaletteOpen: (open: boolean) => void;
  /** Whether the command palette is currently visible. */
  paletteOpen: boolean;
  /** The currently active panel's featureId (null = no panel focused). */
  activeFeatureId: string | null;
}

const ShortcutContext = createContext<ShortcutContextValue | null>(null);

// ── Helpers ─────────────────────────────────────────────────────────

/** Element selectors that indicate a code editor / terminal has focus.
 *  When any of these match the active element, app shortcuts are
 *  suppressed so Monaco/xterm native keybindings take priority. */
const EDITOR_SELECTORS = [
  ".monaco-editor",
  ".monaco-editor *",
  ".xterm",
  ".xterm *",
  ".terminal",
  ".terminal *",
  '[data-editor="true"]',
  '[data-terminal="true"]',
];

function isEditorFocused(): boolean {
  const el = document.activeElement;
  if (!el) return false;
  for (const sel of EDITOR_SELECTORS) {
    if (el.matches(sel) || el.closest(sel)) return true;
  }
  return false;
}

/** Normalize a keyboard event into a canonical shortcut string
 *  (e.g. "Cmd+Shift+P"). Order: modifiers alphabetically, then key. */
function eventToShortcutString(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.metaKey) parts.push("Cmd");
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  // Normalize the key: ignore modifier-only presses
  const key = e.key;
  if (["Meta", "Control", "Alt", "Shift"].includes(key)) return "";
  // Capitalize single letters
  const displayKey = key.length === 1 ? key.toUpperCase() : key;
  parts.push(displayKey);
  return parts.join("+");
}

// ── Provider ────────────────────────────────────────────────────────

export function ShortcutProvider({
  children,
  activeFeatureId = null,
  builtInActions = [],
}: {
  children: ReactNode;
  activeFeatureId?: string | null;
  builtInActions?: CommandAction[];
}) {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const shortcutsRef = useRef<Map<string, ShortcutEntry>>(new Map());
  const [, forceRender] = useState(0);

  const register = useCallback((entry: ShortcutEntry) => {
    shortcutsRef.current.set(entry.id, entry);
    forceRender((n) => n + 1);
  }, []);

  const unregister = useCallback((id: string) => {
    shortcutsRef.current.delete(id);
    forceRender((n) => n + 1);
  }, []);

  // Keyboard event handler
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Never intercept when a code editor / terminal has focus
      if (isEditorFocused()) return;

      const combo = eventToShortcutString(e);
      if (!combo) return;

      // Command palette — always handled here
      if (combo === "Cmd+Shift+P") {
        e.preventDefault();
        setPaletteOpen((prev) => !prev);
        return;
      }

      // Find a matching shortcut
      for (const entry of shortcutsRef.current.values()) {
        for (const keyDef of entry.keys) {
          if (keyDef === combo) {
            // Scope check
            if (entry.scope === "panel" && entry.featureId !== activeFeatureId) {
              continue;
            }
            e.preventDefault();
            entry.action();
            return;
          }
        }
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeFeatureId]);

  const value = useMemo<ShortcutContextValue>(
    () => ({
      register,
      unregister,
      shortcuts: shortcutsRef.current,
      setPaletteOpen,
      paletteOpen,
      activeFeatureId,
    }),
    [register, unregister, paletteOpen, activeFeatureId],
  );

  return (
    <ShortcutContext.Provider value={value}>
      {children}
    </ShortcutContext.Provider>
  );
}

// ── Hook ────────────────────────────────────────────────────────────

export function useShortcuts(): ShortcutContextValue {
  const ctx = useContext(ShortcutContext);
  if (!ctx) {
    throw new Error("useShortcuts must be used within a ShortcutProvider");
  }
  return ctx;
}
