// ── Shortcut registry types ─────────────────────────────────────────
//
// These types define the application-level shortcut system.
// Shortcuts are scoped — 'global' fires regardless of focus,
// 'panel' fires only when the registering feature's panel is active.

import type { ReactNode } from "react";

export type ShortcutScope = "global" | "panel";

/** A single registered shortcut entry. */
export interface ShortcutEntry {
  /** Unique identifier for this shortcut (e.g. "cmd-palette"). */
  id: string;
  /** Human-readable label shown in the command palette. */
  label: string;
  /** The key combo string (e.g. "Cmd+Shift+P", "Cmd+W"). */
  keys: string[];
  /** 'global' = always fires; 'panel' = fires only when feature panel is active. */
  scope: ShortcutScope;
  /** If scoped to a panel, which featureId owns it. */
  featureId?: string;
  /** Optional icon shown in the command palette. */
  icon?: ReactNode;
  /** The action to run when the shortcut is triggered. */
  action: () => void;
}

/** Actions sourced by the command palette from built-ins + features. */
export interface CommandAction {
  id: string;
  label: string;
  shortcut?: string;
  icon?: ReactNode;
  action: () => void;
}
