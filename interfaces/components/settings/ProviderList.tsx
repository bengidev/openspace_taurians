"use client";

import { useMemo, useState } from "react";
import type {
  ActiveProvider,
  Provider,
  ProviderTestError,
  ProviderTestResult,
  TestConnectionErrorKind,
} from "@/lib/types/provider";
import { providerTestConnection } from "@/lib/api/providers";

interface Props {
  providers: Provider[];
  active: ActiveProvider | null;
  onEdit: (provider: Provider) => void;
  onDelete: (id: number) => void;
  onSetActive: (providerId: number, model: string) => void;
}

export function ProviderList({ providers, active, onEdit, onDelete, onSetActive }: Props) {
  if (providers.length === 0) {
    return (
      <div className="text-center py-12 text-gray-500">
        <p className="text-lg">No providers configured</p>
        <p className="text-sm mt-1">Add a provider to get started.</p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {providers.map((provider) => {
        const isActive = active?.provider_id === provider.id;
        const activeModel = isActive ? active.model : null;
        return (
          <ProviderCard
            key={provider.id}
            provider={provider}
            isActive={isActive}
            activeModel={activeModel}
            onEdit={() => onEdit(provider)}
            onDelete={() => onDelete(provider.id)}
            onSetActive={(model) => onSetActive(provider.id, model)}
          />
        );
      })}
    </div>
  );
}

/** Map an error kind to a human-readable label. */
function errorKindLabel(kind: TestConnectionErrorKind): string {
  switch (kind) {
    case "auth":
      return "Authentication failed";
    case "network":
      return "Network error";
    case "invalid_config":
      return "Invalid configuration";
    case "http_status":
      return "Server error";
    case "malformed_response":
      return "Malformed response";
    case "unknown":
      return "Connection failed";
  }
}

/** Map an error kind to a CSS class pair (bg + text). */
function errorKindClasses(kind: TestConnectionErrorKind): string {
  switch (kind) {
    case "auth":
      return "bg-amber-100 text-amber-800";
    case "network":
      return "bg-orange-100 text-orange-800";
    case "invalid_config":
      return "bg-yellow-100 text-yellow-800";
    case "http_status":
      return "bg-purple-100 text-purple-800";
    case "malformed_response":
      return "bg-pink-100 text-pink-800";
    case "unknown":
      return "bg-red-100 text-red-800";
  }
}

function ProviderCard({
  provider,
  isActive,
  activeModel,
  onEdit,
  onDelete,
  onSetActive,
}: {
  provider: Provider;
  isActive: boolean;
  activeModel: string | null;
  onEdit: () => void;
  onDelete: () => void;
  onSetActive: (model: string) => void;
}) {
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);
  const [chosenModel, setChosenModel] = useState(provider.models[0]?.id ?? "");

  const hasModels = provider.models.length > 0;
  const isMissingKey = !provider.has_api_key;
  const selectedModel = useMemo(
    () =>
      isActive && activeModel && provider.models.some((m) => m.id === activeModel)
        ? activeModel
        : chosenModel,
    [isActive, activeModel, provider.models, chosenModel]
  );

  async function handleTest() {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await providerTestConnection(provider.id);
      setTestResult(result);
    } catch (err) {
      setTestResult({
        success: false,
        error: { kind: "unknown", message: String(err) },
      });
    } finally {
      setTesting(false);
    }
  }

  function handleSetModel(model: string) {
    setChosenModel(model);
  }

  return (
    <div
      className={`border rounded-lg p-4 flex items-start gap-4 ${
        isActive ? "border-blue-400 bg-blue-50/40" : ""
      }`}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <h3 className="text-base font-semibold truncate">{provider.name}</h3>
          {isMissingKey && (
            <span className="text-xs px-2 py-0.5 bg-yellow-100 text-yellow-800 rounded">
              No API key
            </span>
          )}
          {isActive && (
            <span className="text-xs font-semibold px-2 py-0.5 bg-blue-600 text-white rounded">
              ✓ Active
            </span>
          )}
        </div>
        <p className="text-sm text-gray-500 truncate font-mono mt-0.5">{provider.base_url}</p>
        <div className="flex flex-wrap gap-1 mt-2">
          {provider.models.map((m) => (
            <span
              key={m.id}
              className={`text-xs px-2 py-0.5 rounded font-mono ${
                isActive && activeModel === m.id
                  ? "bg-blue-200 text-blue-900"
                  : "bg-gray-100 text-gray-700"
              }`}
            >
              {m.id}
            </span>
          ))}
        </div>
        <p className="text-xs text-gray-400 mt-2 font-mono">
          Auth: {provider.auth_header_name}: {provider.auth_header_value_prefix}
          &lt;key&gt;
        </p>

        {hasModels && (
          <div className="flex items-center gap-2 mt-3">
            <select
              value={selectedModel}
              onChange={(e) => handleSetModel(e.target.value)}
              className="text-sm border rounded px-2 py-1 font-mono"
            >
              {provider.models.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name} ({m.id})
                </option>
              ))}
            </select>
            <button
              onClick={() => onSetActive(selectedModel)}
              disabled={isMissingKey || !selectedModel}
              className="text-sm px-3 py-1 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isActive && activeModel === selectedModel ? "Set as Active" : "Set Active"}
            </button>
          </div>
        )}
        {!hasModels && (
          <p className="text-xs text-amber-600 mt-2">
            No models configured — add models to enable selection.
          </p>
        )}
      </div>

      <div className="flex flex-col items-end gap-2 shrink-0">
        <div className="flex gap-2">
          <span
            className="inline-block"
            title={isMissingKey ? "Set an API key before testing" : undefined}
          >
            <button
              onClick={handleTest}
              disabled={testing || isMissingKey}
              className="text-sm px-3 py-1 border rounded hover:bg-gray-50 disabled:opacity-50"
            >
              {testing ? "Testing..." : "Test"}
            </button>
          </span>
          <button
            onClick={onEdit}
            className="text-sm px-3 py-1 border rounded hover:bg-gray-50"
          >
            Edit
          </button>
          <button
            onClick={onDelete}
            className="text-sm px-3 py-1 border border-red-300 text-red-600 rounded hover:bg-red-50"
          >
            Delete
          </button>
        </div>

        {testResult && <TestResultBadge result={testResult} />}
      </div>
    </div>
  );
}

export function TestResultBadge({ result }: { result: ProviderTestResult }) {
  if (result.success) {
    return (
      <div className="text-xs px-2 py-1 rounded mt-1 bg-green-100 text-green-800">
        ✓ Connection OK
      </div>
    );
  }

  const error: ProviderTestError = result.error ?? {
    kind: "unknown",
    message: "Connection failed",
  };

  return (
    <div
      data-testid="test-result-error"
      className={`text-xs px-2 py-1 rounded mt-1 max-w-xs ${errorKindClasses(error.kind)}`}
    >
      <span className="font-semibold">{errorKindLabel(error.kind)}:</span>{" "}
      {error.message}
    </div>
  );
}
