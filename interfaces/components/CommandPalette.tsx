"use client";

import { useCallback, useEffect, useMemo } from "react";
import { Command } from "cmdk";
import { useShortcuts } from "@/contexts/ShortcutContext";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import type { CommandAction } from "@/lib/shortcuts";

// ── Built-in workspace actions ──────────────────────────────────────

function useCommandActions(): CommandAction[] {
  const splitDirection = useWorkspaceStore((s) => s.splitDirection);
  const setSplitDirection = useWorkspaceStore((s) => s.setSplitDirection);
  const openPanel = useWorkspaceStore((s) => s.openPanel);
  const features = useWorkspaceStore((s) => s.listFeatures());

  return useMemo<CommandAction[]>(
    () => [
      {
        id: "workspace-toggle-theme",
        label: "Toggle Theme",
        shortcut: "Cmd+Shift+T",
        icon: <span>🎨</span>,
        action: () => {
          const html = document.documentElement;
          html.classList.toggle("dark");
        },
      },
      {
        id: "workspace-toggle-split",
        label:
          splitDirection === "horizontal"
            ? "Split Vertically"
            : "Split Horizontally",
        shortcut: "Cmd+Shift+S",
        icon: <span>{splitDirection === "horizontal" ? "⬍" : "⬌"}</span>,
        action: () =>
          setSplitDirection(
            splitDirection === "horizontal" ? "vertical" : "horizontal",
          ),
      },
      ...features.map((f) => ({
        id: `workspace-open-${f.featureId}`,
        label: `Open ${f.name}`,
        icon: <span>{f.icon}</span>,
        action: () => openPanel(f.featureId),
      })),
    ],
    [splitDirection, setSplitDirection, openPanel, features],
  );
}

// ── Component ───────────────────────────────────────────────────────

export function CommandPalette() {
  const { paletteOpen, setPaletteOpen, shortcuts } = useShortcuts();
  const builtInActions = useCommandActions();

  // Close on Escape
  useEffect(() => {
    if (!paletteOpen) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") setPaletteOpen(false);
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [paletteOpen, setPaletteOpen]);

  // Merge built-in actions + feature-registered shortcuts into cmd items
  const allActions = useMemo<CommandAction[]>(() => {
    const shortcutActions: CommandAction[] = Array.from(
      shortcuts.values(),
    ).map((s) => ({
      id: s.id,
      label: s.label,
      shortcut: s.keys[0],
      icon: s.icon,
      action: s.action,
    }));
    return [...builtInActions, ...shortcutActions];
  }, [builtInActions, shortcuts]);

  const handleSelect = useCallback(
    (id: string) => {
      const action = allActions.find((a) => a.id === id);
      if (action) {
        action.action();
        setPaletteOpen(false);
      }
    },
    [allActions, setPaletteOpen],
  );

  if (!paletteOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/40"
        onClick={() => setPaletteOpen(false)}
      />
      {/* Palette */}
      <div className="relative w-full max-w-lg rounded-xl border border-zinc-200 bg-white shadow-2xl dark:border-zinc-700 dark:bg-zinc-900">
        <Command label="Command Palette" shouldFilter={true}>
          <div className="flex items-center border-b border-zinc-100 px-3 dark:border-zinc-800">
            <Command.Input
              autoFocus
              placeholder="Type a command or search..."
              className="flex-1 bg-transparent py-3 text-sm text-zinc-800 outline-none placeholder:text-zinc-400 dark:text-zinc-200 dark:placeholder:text-zinc-500"
            />
          </div>
          <Command.List className="max-h-64 overflow-y-auto p-1">
            <Command.Empty className="px-3 py-6 text-center text-sm text-zinc-400 dark:text-zinc-500">
              No results found.
            </Command.Empty>
            <Command.Group heading="Actions" className="px-1">
              {allActions.map((action) => (
                <Command.Item
                  key={action.id}
                  value={action.id}
                  onSelect={() => handleSelect(action.id)}
                  className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-2 text-sm text-zinc-700 aria-selected:bg-zinc-100 dark:text-zinc-300 dark:aria-selected:bg-zinc-800"
                >
                  {action.icon && (
                    <span className="flex h-5 w-5 items-center justify-center text-xs">
                      {action.icon}
                    </span>
                  )}
                  <span className="flex-1">{action.label}</span>
                  {action.shortcut && (
                    <kbd className="ml-auto rounded border border-zinc-200 px-1.5 py-0.5 text-[11px] text-zinc-400 dark:border-zinc-600 dark:text-zinc-500">
                      {action.shortcut}
                    </kbd>
                  )}
                </Command.Item>
              ))}
            </Command.Group>
          </Command.List>
        </Command>
      </div>
    </div>
  );
}
