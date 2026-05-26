"use client";

import { useCallback, useRef, useEffect, useState } from "react";
import { useTranslations } from "next-intl";
import { useWorkspaceStore, type SplitDirection } from "@/stores/workspaceStore";

export function PanelHeader({
  featureId,
  panelId,
}: {
  featureId: string;
  panelId: string;
}) {
  const t = useTranslations();
  const closePanel = useWorkspaceStore((s) => s.closePanel);

  return (
    <div className="flex items-center justify-between h-9 px-3 border-b border-zinc-200 bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-900 shrink-0">
      <span className="text-xs font-medium text-zinc-600 dark:text-zinc-400">
        {t(`feature.${featureId}`)}
      </span>
      <button
        onClick={() => closePanel(panelId)}
        aria-label={t("workspace.panel.close")}
        className="flex items-center justify-center w-5 h-5 rounded text-zinc-400 hover:bg-zinc-200 hover:text-zinc-600 dark:hover:bg-zinc-700 dark:hover:text-zinc-300"
      >
        ×
      </button>
    </div>
  );
}

export function PanelPlaceholder({ featureId }: { featureId: string }) {
  const t = useTranslations();

  return (
    <div className="flex flex-col items-center justify-center flex-1 gap-2 text-zinc-400 dark:text-zinc-500">
      <span className="text-4xl">🚧</span>
      <p className="text-sm">{t("workspace.panel.comingSoon")}</p>
    </div>
  );
}

export function ResizeHandle({
  onResize,
  direction,
}: {
  onResize: (delta: number) => void;
  direction: SplitDirection;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setDragging(true);
      const startPos = direction === "horizontal" ? e.clientX : e.clientY;

      const onMouseMove = (ev: MouseEvent) => {
        const currentPos =
          direction === "horizontal" ? ev.clientX : ev.clientY;
        const delta = currentPos - startPos;
        onResize(delta);
      };

      const onMouseUp = () => {
        setDragging(false);
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);
      };

      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    },
    [onResize, direction],
  );

  const isHorizontal = direction === "horizontal";
  const cursorClass = isHorizontal ? "cursor-col-resize" : "cursor-row-resize";
  const barClass = isHorizontal
    ? "w-1 hover:w-1"
    : "h-1 hover:h-1";

  return (
    <div
      ref={ref}
      onMouseDown={onMouseDown}
      className={`shrink-0 bg-zinc-200 transition-colors hover:bg-blue-400 dark:bg-zinc-700 dark:hover:bg-blue-500 ${cursorClass} ${barClass} ${
        dragging ? "bg-blue-400 dark:bg-blue-500" : ""
      } ${isHorizontal ? "h-full w-1.5" : "w-full h-1.5"}`}
      role="separator"
      aria-orientation={isHorizontal ? "vertical" : "horizontal"}
      aria-label="Resize panel"
    />
  );
}
