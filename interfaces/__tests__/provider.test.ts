import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  providerList,
  providerGet,
  providerCreate,
  providerUpdate,
  providerDelete,
  providerTestConnection,
  activeProviderGet,
  activeProviderSet,
  activeProviderClear,
} from "@/lib/api/providers";
import type { ActiveProvider, Provider, ProviderCreate, ProviderUpdate, ProviderTestResult } from "@/lib/types/provider";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const { invoke } = await import("@tauri-apps/api/core");

describe("Provider API", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("providerList returns array of providers", async () => {
    const mockProviders: Provider[] = [
      {
        id: 1,
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        api_key_redacted: "[REDACTED]",
        has_api_key: true,
        auth_header_name: "Authorization",
        auth_header_value_prefix: "Bearer ",
        models: [
          { id: "gpt-4", name: "GPT-4", context_window: 8192 },
        ],
        request_body_template: { model: "{model}" },
        response_path: "choices[0].message.content",
      },
    ];
    vi.mocked(invoke).mockResolvedValue(mockProviders);

    const result = await providerList();

    expect(invoke).toHaveBeenCalledWith("provider_list");
    expect(result).toEqual(mockProviders);
  });

  it("providerGet returns single provider", async () => {
    const mockProvider: Provider = {
      id: 1,
      name: "OpenAI",
      base_url: "https://api.openai.com/v1",
      api_key_redacted: "[REDACTED]",
      has_api_key: true,
      auth_header_name: "Authorization",
      auth_header_value_prefix: "Bearer ",
      models: [],
      request_body_template: {},
      response_path: "choices[0].message.content",
    };
    vi.mocked(invoke).mockResolvedValue(mockProvider);

    const result = await providerGet(1);

    expect(invoke).toHaveBeenCalledWith("provider_get", { id: 1 });
    expect(result).toEqual(mockProvider);
  });

  it("providerCreate sends config and returns new id", async () => {
    const newProvider: ProviderCreate = {
      name: "Custom Provider",
      base_url: "https://api.example.com/v1",
      api_key: "sk-test-key",
      models: [{ id: "model-1", name: "Model 1", context_window: 4096 }],
      request_body_template: { model: "{model}" },
      response_path: "choices[0].message.content",
    };
    vi.mocked(invoke).mockResolvedValue(42);

    const result = await providerCreate(newProvider);

    expect(invoke).toHaveBeenCalledWith("provider_create", newProvider);
    expect(result).toBe(42);
  });

  it("providerCreate without api_key works", async () => {
    const newProvider: ProviderCreate = {
      name: "Seed Profile",
      base_url: "https://api.example.com/v1",
      models: [],
      request_body_template: {},
      response_path: "choices[0].message.content",
    };
    vi.mocked(invoke).mockResolvedValue(1);

    const result = await providerCreate(newProvider);

    expect(invoke).toHaveBeenCalledWith("provider_create", newProvider);
    expect(result).toBe(1);
  });

  it("providerUpdate sends config with id", async () => {
    const updateConfig: ProviderUpdate = {
      id: 5,
      name: "Updated Provider",
      base_url: "https://api.updated.com/v1",
      api_key: "sk-new-key",
      models: [],
      request_body_template: {},
      response_path: "choices[0].message.content",
    };
    vi.mocked(invoke).mockResolvedValue(true);

    const result = await providerUpdate(updateConfig);

    expect(invoke).toHaveBeenCalledWith("provider_update", updateConfig);
    expect(result).toBe(true);
  });

  it("providerUpdate without api_key preserves existing key", async () => {
    const updateConfig: ProviderUpdate = {
      id: 5,
      name: "Updated Provider",
      base_url: "https://api.updated.com/v1",
      models: [],
      request_body_template: {},
      response_path: "choices[0].message.content",
    };
    vi.mocked(invoke).mockResolvedValue(true);

    const result = await providerUpdate(updateConfig);

    expect(invoke).toHaveBeenCalledWith("provider_update", updateConfig);
    expect(result).toBe(true);
  });

  it("providerDelete sends id and returns boolean", async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    const result = await providerDelete(123);

    expect(invoke).toHaveBeenCalledWith("provider_delete", { id: 123 });
    expect(result).toBe(true);
  });

  it("providerTestConnection returns success result", async () => {
    const testResult: ProviderTestResult = {
      success: true,
      error: undefined,
    };
    vi.mocked(invoke).mockResolvedValue(testResult);

    const result = await providerTestConnection(7);

    expect(invoke).toHaveBeenCalledWith("provider_test_connection", { providerId: 7 });
    expect(result).toEqual(testResult);
  });

  it("providerTestConnection returns error result with structured kind", async () => {
    const testResult: ProviderTestResult = {
      success: false,
      error: { kind: "network", message: "Connection timeout" },
    };
    vi.mocked(invoke).mockResolvedValue(testResult);

    const result = await providerTestConnection(99);

    expect(invoke).toHaveBeenCalledWith("provider_test_connection", { providerId: 99 });
    expect(result).toEqual(testResult);
  });

  it("activeProviderGet returns null when no active provider", async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    const result = await activeProviderGet();

    expect(invoke).toHaveBeenCalledWith("active_provider_get");
    expect(result).toBeNull();
  });

  it("activeProviderGet returns active provider", async () => {
    const mockActive: ActiveProvider = {
      provider_id: 5,
      model: "gpt-4o",
    };
    vi.mocked(invoke).mockResolvedValue(mockActive);

    const result = await activeProviderGet();

    expect(invoke).toHaveBeenCalledWith("active_provider_get");
    expect(result).toEqual(mockActive);
  });

  it("activeProviderSet sends provider_id and model", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);

    await activeProviderSet(7, "claude-3-opus");

    expect(invoke).toHaveBeenCalledWith("active_provider_set", {
      providerId: 7,
      model: "claude-3-opus",
    });
  });

  it("activeProviderClear returns boolean", async () => {
    vi.mocked(invoke).mockResolvedValue(true);

    const result = await activeProviderClear();

    expect(invoke).toHaveBeenCalledWith("active_provider_clear");
    expect(result).toBe(true);
  });

  it("activeProviderClear returns false when no active provider", async () => {
    vi.mocked(invoke).mockResolvedValue(false);

    const result = await activeProviderClear();

    expect(invoke).toHaveBeenCalledWith("active_provider_clear");
    expect(result).toBe(false);
  });
});
