"use client";

import { useTranslations } from "next-intl";
import { useWorkspaceStore } from "@/stores/workspaceStore";

export function Sidebar() {
  const t = useTranslations();
  const features = useWorkspaceStore((s) => s.listFeatures());
  const openPanel = useWorkspaceStore((s) => s.openPanel);
  const panels = useWorkspaceStore((s) => s.panels);
  const activePanelId = useWorkspaceStore((s) => s.activePanelId);

  const isPanelOpen = (featureId: string) =>
    panels.some((p) => p.featureId === featureId);

  return (
    <aside className="flex flex-col w-14 border-r border-zinc-200 bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-900">
      <nav className="flex flex-col items-center gap-1 py-2">
        {features.map((f) => {
          const open = isPanelOpen(f.featureId);
          const active = panels.some(
            (p) => p.featureId === f.featureId && p.panelId === activePanelId,
          );
          return (
            <button
              key={f.featureId}
              onClick={() => openPanel(f.featureId)}
              title={t(`feature.${f.featureId}`)}
              aria-label={t("workspace.sidebar.openPanel")}
              className={`flex flex-col items-center justify-center w-10 h-10 rounded-md text-xs transition-colors ${
                active
                  ? "bg-zinc-200 text-zinc-900 dark:bg-zinc-700 dark:text-zinc-100"
                  : open
                    ? "text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800"
                    : "text-zinc-400 hover:bg-zinc-100 dark:text-zinc-500 dark:hover:bg-zinc-800"
              }`}
            >
              <span className="text-lg leading-none">{f.icon}</span>
              <span className="mt-0.5 text-[10px] leading-none">
                {f.featureId.slice(0, 4)}
              </span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
}
