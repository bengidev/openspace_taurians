import { describe, it, expect, beforeEach } from "vitest";
import { useWorkspaceStore } from "@/stores/workspaceStore";

// Reset store state between tests.
beforeEach(() => {
  localStorage.clear();
  useWorkspaceStore.setState({
    panels: [],
    activePanelId: null,
    splitDirection: "horizontal",
  });
});

describe("workspaceStore", () => {
  describe("openPanel", () => {
    it("adds a panel and sets it as active", () => {
      const store = useWorkspaceStore.getState();

      store.openPanel("editor");

      const state = useWorkspaceStore.getState();
      expect(state.panels).toHaveLength(1);
      expect(state.panels[0].featureId).toBe("editor");
      expect(state.activePanelId).toBe(state.panels[0].panelId);
    });

    it("adds multiple panels; last opened is active", () => {
      const store = useWorkspaceStore.getState();

      store.openPanel("editor");
      store.openPanel("terminal");

      const state = useWorkspaceStore.getState();
      expect(state.panels).toHaveLength(2);
      expect(state.activePanelId).toBe(state.panels[1].panelId);
    });
  });

  describe("closePanel", () => {
    it("removes the panel", () => {
      const store = useWorkspaceStore.getState();
      store.openPanel("editor");
      const { panelId } = useWorkspaceStore.getState().panels[0];

      store.closePanel(panelId);

      const state = useWorkspaceStore.getState();
      expect(state.panels).toHaveLength(0);
      expect(state.activePanelId).toBeNull();
    });

    it("activates the previous panel when active one is closed", () => {
      const store = useWorkspaceStore.getState();
      store.openPanel("editor");
      store.openPanel("terminal");

      const panels = useWorkspaceStore.getState().panels;
      const firstPanelId = panels[0].panelId;
      const secondPanelId = panels[1].panelId;

      // Close the active (second) panel — first should become active.
      store.closePanel(secondPanelId);

      const state = useWorkspaceStore.getState();
      expect(state.panels).toHaveLength(1);
      expect(state.activePanelId).toBe(firstPanelId);
    });
  });

  describe("focusPanel", () => {
    it("changes the active panel", () => {
      const store = useWorkspaceStore.getState();
      store.openPanel("editor");
      store.openPanel("terminal");

      const firstPanelId = useWorkspaceStore.getState().panels[0].panelId;

      store.focusPanel(firstPanelId);

      expect(useWorkspaceStore.getState().activePanelId).toBe(firstPanelId);
    });
  });

  describe("resizePanel", () => {
    it("updates panel size", () => {
      const store = useWorkspaceStore.getState();
      store.openPanel("editor");
      const { panelId } = useWorkspaceStore.getState().panels[0];

      store.resizePanel(panelId, 600);

      const panel = useWorkspaceStore.getState().panels[0];
      expect(panel.size).toBe(600);
    });

    it("clamps size to minimum 100", () => {
      const store = useWorkspaceStore.getState();
      store.openPanel("editor");
      const { panelId } = useWorkspaceStore.getState().panels[0];

      store.resizePanel(panelId, 50);

      const panel = useWorkspaceStore.getState().panels[0];
      expect(panel.size).toBe(100);
    });
  });

  describe("setSplitDirection", () => {
    it("toggles split direction", () => {
      const store = useWorkspaceStore.getState();
      expect(store.splitDirection).toBe("horizontal");

      store.setSplitDirection("vertical");
      expect(useWorkspaceStore.getState().splitDirection).toBe("vertical");
    });
  });

  describe("listFeatures", () => {
    it("returns built-in features", () => {
      const features = useWorkspaceStore.getState().listFeatures();
      expect(features).toHaveLength(5);
      expect(features[0].featureId).toBe("editor");
      expect(features[4].featureId).toBe("settings");
    });
  });

  describe("persistence", () => {
    it("partialize excludes transient state", () => {
      // Open a panel to create non-empty state.
      useWorkspaceStore.getState().openPanel("editor");

      // Simulate what persist middleware would save.
      const partialState = useWorkspaceStore.persist.getOptions().partialize?.(
        useWorkspaceStore.getState(),
      );

      expect(partialState).toBeDefined();
      expect(partialState).toHaveProperty("panels");
      expect(partialState).toHaveProperty("activePanelId");
      expect(partialState).toHaveProperty("splitDirection");
      // openPanel/closePanel etc. (functions) should NOT be persisted.
      expect(partialState).not.toHaveProperty("openPanel");
    });
  });
});
