"use client";

import { useRegisterShortcut } from "@/hooks/useRegisterShortcut";
import { useWorkspaceStore } from "@/stores/workspaceStore";

/**
 * Registers workspace-level built-in shortcuts that operate on
 * the workspace store: Cmd+W to close the active panel,
 * Cmd+1..9 to switch to the Nth panel.
 */
export function WorkspaceShortcuts() {
  const closePanel = useWorkspaceStore((s) => s.closePanel);
  const focusPanel = useWorkspaceStore((s) => s.focusPanel);
  const activePanelId = useWorkspaceStore((s) => s.activePanelId);
  const panels = useWorkspaceStore((s) => s.panels);

  // Cmd+W — close the currently active panel
  useRegisterShortcut({
    id: "workspace-close-panel",
    label: "Close Active Panel",
    keys: ["Cmd+W"],
    scope: "global",
    action: () => {
      if (activePanelId) closePanel(activePanelId);
    },
  });

  // Cmd+1 — switch to panel 1
  useRegisterShortcut({
    id: "workspace-switch-panel-1",
    label: "Switch to Panel 1",
    keys: ["Cmd+1"],
    scope: "global",
    action: () => { const p = panels[0]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+2
  useRegisterShortcut({
    id: "workspace-switch-panel-2",
    label: "Switch to Panel 2",
    keys: ["Cmd+2"],
    scope: "global",
    action: () => { const p = panels[1]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+3
  useRegisterShortcut({
    id: "workspace-switch-panel-3",
    label: "Switch to Panel 3",
    keys: ["Cmd+3"],
    scope: "global",
    action: () => { const p = panels[2]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+4
  useRegisterShortcut({
    id: "workspace-switch-panel-4",
    label: "Switch to Panel 4",
    keys: ["Cmd+4"],
    scope: "global",
    action: () => { const p = panels[3]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+5
  useRegisterShortcut({
    id: "workspace-switch-panel-5",
    label: "Switch to Panel 5",
    keys: ["Cmd+5"],
    scope: "global",
    action: () => { const p = panels[4]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+6
  useRegisterShortcut({
    id: "workspace-switch-panel-6",
    label: "Switch to Panel 6",
    keys: ["Cmd+6"],
    scope: "global",
    action: () => { const p = panels[5]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+7
  useRegisterShortcut({
    id: "workspace-switch-panel-7",
    label: "Switch to Panel 7",
    keys: ["Cmd+7"],
    scope: "global",
    action: () => { const p = panels[6]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+8
  useRegisterShortcut({
    id: "workspace-switch-panel-8",
    label: "Switch to Panel 8",
    keys: ["Cmd+8"],
    scope: "global",
    action: () => { const p = panels[7]; if (p) focusPanel(p.panelId); },
  });
  // Cmd+9
  useRegisterShortcut({
    id: "workspace-switch-panel-9",
    label: "Switch to Panel 9",
    keys: ["Cmd+9"],
    scope: "global",
    action: () => { const p = panels[8]; if (p) focusPanel(p.panelId); },
  });

  return null;
}
