import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, fireEvent, screen } from "@testing-library/react";
import { ShortcutProvider } from "@/contexts/ShortcutContext";
import { useRegisterShortcut } from "@/hooks/useRegisterShortcut";

// ── Test helper component ──────────────────────────────────────────

interface TestHarnessProps {
  /** FeatureId to simulate as "active" for scoping tests. */
  activeFeatureId?: string | null;
  /** Callback invoked when a test shortcut fires. */
  onFire?: (id: string) => void;
  /** Which shortcuts to register */
  shortcuts?: Array<{
    id: string;
    label: string;
    keys: string[];
    scope: "global" | "panel";
    featureId?: string;
  }>;
}

function TestHarness({
  activeFeatureId = null,
  onFire,
  shortcuts = [],
}: TestHarnessProps) {
  return (
    <ShortcutProvider activeFeatureId={activeFeatureId}>
      {shortcuts.map((s) => (
        <ShortcutRegistrar
          key={s.id}
          id={s.id}
          label={s.label}
          keys={s.keys}
          scope={s.scope}
          featureId={s.featureId}
          onFire={onFire}
        />
      ))}
      <div data-testid="target" tabIndex={0}>
        focus me
      </div>
    </ShortcutProvider>
  );
}

function ShortcutRegistrar({
  id,
  label,
  keys,
  scope,
  featureId,
  onFire,
}: {
  id: string;
  label: string;
  keys: string[];
  scope: "global" | "panel";
  featureId?: string;
  onFire?: (id: string) => void;
}) {
  useRegisterShortcut({
    id,
    label,
    keys,
    scope,
    featureId,
    action: () => onFire?.(id),
  });
  return null;
}

// ── Helpers ────────────────────────────────────────────────────────

/** Fire a synthetic keydown event on the window. */
function pressKeys(combo: string) {
  const parts = combo.split("+");
  const key = parts.pop()!;
  fireEvent.keyDown(window, {
    key,
    metaKey: parts.includes("Cmd"),
    ctrlKey: parts.includes("Ctrl"),
    altKey: parts.includes("Alt"),
    shiftKey: parts.includes("Shift"),
  });
}

// ── Tests ──────────────────────────────────────────────────────────

describe("shortcut system", () => {
  describe("registration and firing", () => {
    it("fires a registered global shortcut", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "test-action",
              label: "Test Action",
              keys: ["Cmd+K"],
              scope: "global",
            },
          ]}
        />,
      );

      pressKeys("Cmd+K");

      expect(onFire).toHaveBeenCalledWith("test-action");
    });

    it("does not fire when no shortcut matches", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "test-action",
              label: "Test",
              keys: ["Cmd+K"],
              scope: "global",
            },
          ]}
        />,
      );

      pressKeys("Cmd+J");

      expect(onFire).not.toHaveBeenCalled();
    });

    it("fires multiple shortcuts registered with different keys", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "action-a",
              label: "A",
              keys: ["Cmd+1"],
              scope: "global",
            },
            {
              id: "action-b",
              label: "B",
              keys: ["Cmd+2"],
              scope: "global",
            },
          ]}
        />,
      );

      pressKeys("Cmd+1");
      expect(onFire).toHaveBeenCalledWith("action-a");

      pressKeys("Cmd+2");
      expect(onFire).toHaveBeenCalledWith("action-b");
    });
  });

  describe("scoping", () => {
    it("fires panel-scoped shortcut when featureId matches active", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          activeFeatureId="editor"
          onFire={onFire}
          shortcuts={[
            {
              id: "editor-save",
              label: "Save",
              keys: ["Cmd+S"],
              scope: "panel",
              featureId: "editor",
            },
          ]}
        />,
      );

      pressKeys("Cmd+S");

      expect(onFire).toHaveBeenCalledWith("editor-save");
    });

    it("does NOT fire panel-scoped shortcut when featureId mismatches", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          activeFeatureId="terminal"
          onFire={onFire}
          shortcuts={[
            {
              id: "editor-save",
              label: "Save",
              keys: ["Cmd+S"],
              scope: "panel",
              featureId: "editor",
            },
          ]}
        />,
      );

      pressKeys("Cmd+S");

      expect(onFire).not.toHaveBeenCalled();
    });

    it("global shortcut fires regardless of activeFeatureId", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          activeFeatureId="terminal"
          onFire={onFire}
          shortcuts={[
            {
              id: "toggle-sidebar",
              label: "Toggle Sidebar",
              keys: ["Cmd+B"],
              scope: "global",
            },
          ]}
        />,
      );

      pressKeys("Cmd+B");

      expect(onFire).toHaveBeenCalledWith("toggle-sidebar");
    });

    it("global shortcut fires when activeFeatureId is null", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          activeFeatureId={null}
          onFire={onFire}
          shortcuts={[
            {
              id: "toggle-sidebar",
              label: "Toggle Sidebar",
              keys: ["Cmd+B"],
              scope: "global",
            },
          ]}
        />,
      );

      pressKeys("Cmd+B");

      expect(onFire).toHaveBeenCalledWith("toggle-sidebar");
    });
  });

  describe("editor/terminal focus bypass", () => {
    it("does NOT fire shortcuts when .monaco-editor child is focused", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "cmd-save",
              label: "Save",
              keys: ["Cmd+S"],
              scope: "global",
            },
          ]}
        />,
      );

      // Simulate focus inside a Monaco editor
      const editorEl = document.createElement("div");
      editorEl.className = "monaco-editor";
      editorEl.setAttribute("data-testid", "monaco");
      document.body.appendChild(editorEl);

      const inner = document.createElement("textarea");
      editorEl.appendChild(inner);
      inner.focus();

      pressKeys("Cmd+S");

      expect(onFire).not.toHaveBeenCalled();

      document.body.removeChild(editorEl);
    });

    it("does NOT fire shortcuts when .xterm child is focused", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "global-action",
              label: "Global",
              keys: ["Cmd+G"],
              scope: "global",
            },
          ]}
        />,
      );

      // Simulate focus inside xterm.js
      const termEl = document.createElement("div");
      termEl.className = "xterm";
      document.body.appendChild(termEl);

      const inner = document.createElement("div");
      termEl.appendChild(inner);
      inner.tabIndex = 0;
      inner.focus();

      pressKeys("Cmd+G");

      expect(onFire).not.toHaveBeenCalled();

      document.body.removeChild(termEl);
    });

    it("DOES fire shortcuts when a regular element is focused", () => {
      const onFire = vi.fn();
      render(
        <TestHarness
          onFire={onFire}
          shortcuts={[
            {
              id: "global-action",
              label: "Global",
              keys: ["Cmd+G"],
              scope: "global",
            },
          ]}
        />,
      );

      screen.getByTestId("target").focus();

      pressKeys("Cmd+G");

      expect(onFire).toHaveBeenCalledWith("global-action");
    });
  });
});
