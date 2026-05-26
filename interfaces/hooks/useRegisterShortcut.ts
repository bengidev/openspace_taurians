"use client";

import { useEffect } from "react";
import { useShortcuts } from "@/contexts/ShortcutContext";
import type { ShortcutEntry } from "@/lib/shortcuts";

/**
 * Register an application-level shortcut for the lifetime of the
 * calling component. Unregisters on unmount.
 *
 * Usage (inside a feature panel):
 *   useRegisterShortcut({
 *     id: "editor-format",
 *     label: "Format Document",
 *     keys: ["Cmd+Shift+F"],
 *     scope: "panel",
 *     featureId: "editor",
 *     action: () => formatDocument(),
 *   });
 */
export function useRegisterShortcut(entry: ShortcutEntry): void {
  const { register, unregister } = useShortcuts();

  useEffect(() => {
    register(entry);
    return () => unregister(entry.id);
  }, [entry.id, entry.keys.join(","), entry.scope, entry.featureId]);
}
