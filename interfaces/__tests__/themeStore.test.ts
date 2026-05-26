import { describe, it, expect, beforeEach } from "vitest";
import { useThemeStore } from "@/stores/themeStore";

// Reset store state between tests.
beforeEach(() => {
  localStorage.clear();
  useThemeStore.setState({
    mode: "dark",
    accentColor: "#3b82f6",
  });
});

describe("themeStore", () => {
  describe("initial state", () => {
    it("starts with dark mode by default", () => {
      expect(useThemeStore.getState().mode).toBe("dark");
    });

    it("has the default accent color", () => {
      expect(useThemeStore.getState().accentColor).toBe("#3b82f6");
    });
  });

  describe("toggle", () => {
    it("switches from dark to light", () => {
      const store = useThemeStore.getState();
      expect(store.mode).toBe("dark");

      store.toggle();

      expect(useThemeStore.getState().mode).toBe("light");
    });

    it("switches from light to dark", () => {
      useThemeStore.setState({ mode: "light" });

      useThemeStore.getState().toggle();

      expect(useThemeStore.getState().mode).toBe("dark");
    });

    it("toggles back and forth correctly", () => {
      const store = useThemeStore.getState();

      store.toggle();
      expect(useThemeStore.getState().mode).toBe("light");

      store.toggle();
      expect(useThemeStore.getState().mode).toBe("dark");

      store.toggle();
      expect(useThemeStore.getState().mode).toBe("light");
    });
  });

  describe("setMode", () => {
    it("sets mode to dark", () => {
      useThemeStore.setState({ mode: "light" });

      useThemeStore.getState().setMode("dark");

      expect(useThemeStore.getState().mode).toBe("dark");
    });

    it("sets mode to light", () => {
      useThemeStore.getState().setMode("light");

      expect(useThemeStore.getState().mode).toBe("light");
    });

    it("is idempotent when set to current mode", () => {
      useThemeStore.getState().setMode("dark");
      useThemeStore.getState().setMode("dark");

      expect(useThemeStore.getState().mode).toBe("dark");
    });
  });

  describe("setAccent", () => {
    it("updates the accent color", () => {
      useThemeStore.getState().setAccent("#ef4444");

      expect(useThemeStore.getState().accentColor).toBe("#ef4444");
    });

    it("does not change mode when setting accent", () => {
      useThemeStore.getState().setAccent("#22c55e");

      expect(useThemeStore.getState().mode).toBe("dark");
    });
  });

  describe("persistence", () => {
    it("partialize only includes mode and accentColor", () => {
      const partialState = useThemeStore.persist.getOptions().partialize?.(
        useThemeStore.getState(),
      );

      expect(partialState).toBeDefined();
      expect(partialState).toHaveProperty("mode");
      expect(partialState).toHaveProperty("accentColor");
      // Functions should NOT be persisted.
      expect(partialState).not.toHaveProperty("toggle");
      expect(partialState).not.toHaveProperty("setMode");
      expect(partialState).not.toHaveProperty("setAccent");
    });

    it("persists mode across localStorage round-trip", () => {
      // Toggle to light mode.
      useThemeStore.getState().toggle();
      expect(useThemeStore.getState().mode).toBe("light");

      // Read back from the persist middleware's storage.
      const stored = useThemeStore.persist.getOptions().storage?.getItem(
        "openspace-theme",
      );

      expect(stored).toBeDefined();
      // The stored value should contain mode: "light".
      expect(JSON.stringify(stored)).toContain('"mode":"light"');
    });

    it("persists accentColor across localStorage round-trip", () => {
      useThemeStore.getState().setAccent("#f59e0b");

      const stored = useThemeStore.persist.getOptions().storage?.getItem(
        "openspace-theme",
      );

      expect(stored).toBeDefined();
      expect(JSON.stringify(stored)).toContain('"accentColor":"#f59e0b"');
    });
  });
});
