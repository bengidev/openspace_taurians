"use client";

import { ShortcutProvider } from "@/contexts/ShortcutContext";
import { CommandPalette } from "@/components/CommandPalette";
import { WorkspaceShortcuts } from "@/components/WorkspaceShortcuts";
import WorkspaceLayout from "@/components/WorkspaceLayout";
import { useWorkspaceStore } from "@/stores/workspaceStore";

function WorkspaceWithShortcuts({ children }: { children: React.ReactNode }) {
  const activePanelId = useWorkspaceStore((s) => s.activePanelId);
  const panels = useWorkspaceStore((s) => s.panels);

  // Resolve the active panel's featureId for scope checking
  const activeFeatureId =
    panels.find((p) => p.panelId === activePanelId)?.featureId ?? null;

  return (
    <ShortcutProvider activeFeatureId={activeFeatureId}>
      <WorkspaceShortcuts />
      <CommandPalette />
      <WorkspaceLayout>{children}</WorkspaceLayout>
    </ShortcutProvider>
  );
}

export default function WorkspaceRootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <WorkspaceWithShortcuts>{children}</WorkspaceWithShortcuts>;
}
