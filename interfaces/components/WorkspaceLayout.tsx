"use client";

import { useCallback } from "react";
import { useTranslations } from "next-intl";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { Sidebar } from "@/components/Sidebar";
import { PanelHeader, PanelPlaceholder, ResizeHandle } from "@/components/Panel";
import { ThemeToggle } from "@/components/ThemeToggle";

export default function WorkspaceLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const t = useTranslations();
  const panels = useWorkspaceStore((s) => s.panels);
  const activePanelId = useWorkspaceStore((s) => s.activePanelId);
  const splitDirection = useWorkspaceStore((s) => s.splitDirection);
  const focusPanel = useWorkspaceStore((s) => s.focusPanel);
  const resizePanel = useWorkspaceStore((s) => s.resizePanel);
  const setSplitDirection = useWorkspaceStore((s) => s.setSplitDirection);

  const isHorizontal = splitDirection === "horizontal";

  const handleResize = useCallback(
    (panelId: string) => (delta: number) => {
      const panel = panels.find((p) => p.panelId === panelId);
      if (panel) {
        const newSize = (panel.size ?? 400) + delta;
        resizePanel(panelId, newSize);
      }
    },
    [panels, resizePanel],
  );

  const toggleSplit = () => {
    setSplitDirection(isHorizontal ? "vertical" : "horizontal");
  };

  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="flex flex-col flex-1 min-w-0">
        {/* Toolbar */}
        <div className="flex items-center justify-between h-9 px-3 border-b border-zinc-200 bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-900 shrink-0">
          <span className="text-xs font-medium text-zinc-500 dark:text-zinc-400">
            OpenSpace
          </span>
          <button
            onClick={toggleSplit}
            aria-label={
              isHorizontal
                ? t("workspace.panel.splitVertical")
                : t("workspace.panel.splitHorizontal")
            }
            title={
              isHorizontal
                ? t("workspace.panel.splitVertical")
                : t("workspace.panel.splitHorizontal")
            }
            className="flex items-center gap-1 px-2 py-0.5 text-xs rounded text-zinc-500 hover:bg-zinc-200 dark:text-zinc-400 dark:hover:bg-zinc-700"
          >
            {isHorizontal ? "⬍" : "⬌"}
          </button>
          <ThemeToggle />
        </div>

        {/* Panel area */}
        {panels.length === 0 ? (
          <div className="flex flex-col items-center justify-center flex-1 gap-2 text-zinc-400 dark:text-zinc-500">
            <p className="text-sm font-medium">{t("workspace.empty.title")}</p>
            <p className="text-xs">{t("workspace.empty.description")}</p>
          </div>
        ) : (
          <div
            className={`flex flex-1 min-h-0 ${
              isHorizontal ? "flex-row" : "flex-col"
            }`}
          >
            {panels.map((panel, i) => (
              <div
                key={panel.panelId}
                onClick={() => focusPanel(panel.panelId)}
                className={`flex flex-col min-w-0 min-h-0 ${
                  activePanelId === panel.panelId
                    ? "ring-1 ring-inset ring-blue-400 dark:ring-blue-500"
                    : ""
                } ${isHorizontal ? "flex-1" : "flex-1"}`}
                style={{
                  flexBasis: panel.size ? `${panel.size}px` : undefined,
                  flexGrow: panel.size ? 0 : 1,
                  flexShrink: panel.size ? 0 : 1,
                }}
              >
                <PanelHeader
                  featureId={panel.featureId}
                  panelId={panel.panelId}
                />
                <div className="flex-1 overflow-auto">
                  <PanelPlaceholder featureId={panel.featureId} />
                </div>
                {/* Render the page slot as a child for real content */}
                <div className="hidden">{children}</div>
              </div>
            ))}
            {/* Resize handles between panels */}
            {panels.length > 1 &&
              panels.slice(0, -1).map((panel, i) => (
                <ResizeHandle
                  key={`resize-${panel.panelId}`}
                  onResize={handleResize(panel.panelId)}
                  direction={splitDirection}
                />
              ))}
          </div>
        )}
      </main>
    </div>
  );
}
