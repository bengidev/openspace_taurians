"use client";

import { useTranslations } from "next-intl";
import { ChatPanel } from "@/components/chat/ChatPanel";

export function FeaturePanelContent({
  featureName,
}: {
  featureName: string;
}) {
  const t = useTranslations();

  if (featureName === "chat") {
    return <ChatPanel />;
  }

  return (
    <div className="flex flex-col items-center justify-center h-full gap-2 text-zinc-400 dark:text-zinc-500">
      <span className="text-4xl">🚧</span>
      <p className="text-sm">{t("workspace.panel.comingSoon")}</p>
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        {t(`feature.${featureName}`)}
      </p>
    </div>
  );
}
