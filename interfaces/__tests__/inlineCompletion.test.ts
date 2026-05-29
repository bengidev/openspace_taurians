import { describe, it, expect, vi, beforeEach, Mock } from "vitest";
import { inlineCompletion } from "../lib/api/providers";
import type { InlineCompletionResponse } from "../lib/types/provider";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("inlineCompletion", () => {
  let mockInvoke: Mock;

  beforeEach(async () => {
    vi.clearAllMocks();
    const tauriCore = await import("@tauri-apps/api/core");
    mockInvoke = tauriCore.invoke as Mock;
  });

  it("sends request through inline_completion command", async () => {
    const mockResponse: InlineCompletionResponse = {
      completion: "let x = 42;",
      provider_name: "OpenAI",
      model: "gpt-4o",
    };
    mockInvoke.mockResolvedValue(mockResponse);

    const result = await inlineCompletion({
      document: "fn main() {\n    let x = ",
      cursor_position: 23,
    });

    expect(mockInvoke).toHaveBeenCalledWith("inline_completion", {
      request: {
        document: "fn main() {\n    let x = ",
        cursor_position: 23,
      },
    });
    expect(result).toEqual(mockResponse);
  });

  it("returns completion text from the provider", async () => {
    mockInvoke.mockResolvedValue({
      completion: "println!(\"hello\");",
      provider_name: "Anthropic",
      model: "claude-sonnet-4-20250514",
    });

    const result = await inlineCompletion({
      document: "fn greet() {\n    ",
      cursor_position: 17,
    });

    expect(result.completion).toBe('println!("hello");');
    expect(result.provider_name).toBe("Anthropic");
    expect(result.model).toBe("claude-sonnet-4-20250514");
  });

  it("propagates error when no active provider is configured", async () => {
    mockInvoke.mockRejectedValue(
      new Error(
        "No active provider configured. Open Settings → Providers to choose a provider and model.",
      ),
    );

    await expect(
      inlineCompletion({ document: "code", cursor_position: 0 }),
    ).rejects.toThrow("No active provider configured");
  });

  it("propagates provider errors (auth failure, response path, etc.)", async () => {
    mockInvoke.mockRejectedValue(
      new Error("provider returned HTTP 401: Unauthorized"),
    );

    await expect(
      inlineCompletion({ document: "code", cursor_position: 0 }),
    ).rejects.toThrow("HTTP 401");
  });

  it("propagates response path extraction errors", async () => {
    mockInvoke.mockRejectedValue(
      new Error("response path 'choices[0].message.content' did not resolve to a string"),
    );

    await expect(
      inlineCompletion({ document: "code", cursor_position: 0 }),
    ).rejects.toThrow("response path");
  });

  it("handles empty document correctly", async () => {
    mockInvoke.mockResolvedValue({
      completion: "// start here",
      provider_name: "OpenAI",
      model: "gpt-4o",
    });

    const result = await inlineCompletion({
      document: "",
      cursor_position: 0,
    });

    expect(mockInvoke).toHaveBeenCalledWith("inline_completion", {
      request: { document: "", cursor_position: 0 },
    });
    expect(result.completion).toBe("// start here");
  });
});
