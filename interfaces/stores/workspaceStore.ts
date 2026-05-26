"use client";

import { create } from "zustand";
import { persist } from "zustand/middleware";

// ── Types ──────────────────────────────────────────────────────────

export interface FeatureMeta {
  featureId: string;
  name: string;
  icon: string;
}

export interface PanelState {
  featureId: string;
  panelId: string;
  size?: number;
}

export type SplitDirection = "horizontal" | "vertical";

export interface WorkspaceState {
  panels: PanelState[];
  activePanelId: string | null;
  splitDirection: SplitDirection;

  // Actions
  openPanel: (featureId: string) => void;
  closePanel: (panelId: string) => void;
  focusPanel: (panelId: string) => void;
  resizePanel: (panelId: string, size: number) => void;
  setSplitDirection: (dir: SplitDirection) => void;
  listFeatures: () => FeatureMeta[];
}

// ── Built-in features ──────────────────────────────────────────────

const BUILT_IN_FEATURES: FeatureMeta[] = [
  { featureId: "editor", name: "Editor", icon: "📝" },
  { featureId: "terminal", name: "Terminal", icon: "💻" },
  { featureId: "chat", name: "Chat", icon: "💬" },
  { featureId: "git", name: "Git", icon: "🔀" },
  { featureId: "settings", name: "Settings", icon: "⚙️" },
];

// ── Helpers ────────────────────────────────────────────────────────

let panelCounter = 0;
function nextPanelId(): string {
  panelCounter += 1;
  return `panel-${Date.now()}-${panelCounter}`;
}

// ── Store ──────────────────────────────────────────────────────────

export const useWorkspaceStore = create<WorkspaceState>()(
  persist(
    (set, get) => ({
      panels: [],
      activePanelId: null,
      splitDirection: "horizontal",

      openPanel: (featureId: string) => {
        const panelId = nextPanelId();
        set((state) => ({
          panels: [...state.panels, { featureId, panelId }],
          activePanelId: panelId,
        }));
      },

      closePanel: (panelId: string) => {
        set((state) => {
          const panels = state.panels.filter((p) => p.panelId !== panelId);
          const activePanelId =
            state.activePanelId === panelId
              ? panels.length > 0
                ? panels[panels.length - 1].panelId
                : null
              : state.activePanelId;
          return { panels, activePanelId };
        });
      },

      focusPanel: (panelId: string) => {
        set({ activePanelId: panelId });
      },

      resizePanel: (panelId: string, size: number) => {
        set((state) => ({
          panels: state.panels.map((p) =>
            p.panelId === panelId ? { ...p, size: Math.max(100, size) } : p,
          ),
        }));
      },

      setSplitDirection: (dir: SplitDirection) => {
        set({ splitDirection: dir });
      },

      listFeatures: () => BUILT_IN_FEATURES,
    }),
    {
      name: "openspace-workspace",
      // Only persist panel layout, not transient state
      partialize: (state) => ({
        panels: state.panels,
        activePanelId: state.activePanelId,
        splitDirection: state.splitDirection,
      }),
    },
  ),
);
