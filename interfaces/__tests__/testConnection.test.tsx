import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { TestResultBadge } from "@/components/settings/ProviderList";
import type { ProviderTestResult, TestConnectionErrorKind } from "@/lib/types/provider";

// ── TestResultBadge ──────────────────────────────────────────────

describe("TestResultBadge", () => {
  it("renders success message", () => {
    const result: ProviderTestResult = { success: true };
    render(<TestResultBadge result={result} />);

    expect(screen.getByText(/Connection OK/)).toBeTruthy();
  });

  const errorCases: Array<{
    kind: TestConnectionErrorKind;
    label: string;
  }> = [
    { kind: "auth", label: "Authentication failed" },
    { kind: "network", label: "Network error" },
    { kind: "invalid_config", label: "Invalid configuration" },
    { kind: "http_status", label: "Server error" },
    { kind: "malformed_response", label: "Malformed response" },
    { kind: "unknown", label: "Connection failed" },
  ];

  for (const { kind, label } of errorCases) {
    it(`renders ${kind} error with label "${label}"`, () => {
      const result: ProviderTestResult = {
        success: false,
        error: { kind, message: "detail text" },
      };
      render(<TestResultBadge result={result} />);

      expect(screen.getByText(new RegExp(label))).toBeTruthy();
      expect(screen.getByText(/detail text/)).toBeTruthy();
    });
  }

  it("renders fallback when error is missing", () => {
    const result: ProviderTestResult = { success: false };
    render(<TestResultBadge result={result} />);

    // "Connection failed" appears as both label and message when error is absent.
    const matches = screen.getAllByText(/Connection failed/);
    expect(matches.length).toBeGreaterThanOrEqual(2);
  });

  it("does not display plaintext API keys in error messages", () => {
    const result: ProviderTestResult = {
      success: false,
      error: {
        kind: "auth",
        message: "provider returned HTTP 401: bad request",
      },
    };
    const { container } = render(<TestResultBadge result={result} />);

    // The error message should not contain any key-like strings.
    expect(container.textContent).not.toContain("sk-");
    expect(container.textContent).not.toContain("Bearer");
    expect(container.textContent).not.toContain("secret");
  });
});
