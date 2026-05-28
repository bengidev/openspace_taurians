"use client";

import { useState } from "react";
import type { Provider, ProviderTestResult } from "@/lib/types/provider";
import { providerTestConnection } from "@/lib/api/providers";

interface Props {
  providers: Provider[];
  onEdit: (provider: Provider) => void;
  onDelete: (id: number) => void;
}

export function ProviderList({ providers, onEdit, onDelete }: Props) {
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
      {providers.map((provider) => (
        <ProviderCard
          key={provider.id}
          provider={provider}
          onEdit={() => onEdit(provider)}
          onDelete={() => onDelete(provider.id)}
        />
      ))}
    </div>
  );
}

function ProviderCard({
  provider,
  onEdit,
  onDelete,
}: {
  provider: Provider;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ProviderTestResult | null>(null);

  async function handleTest() {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await providerTestConnection(provider.id);
      setTestResult(result);
    } catch (err) {
      setTestResult({ success: false, error: String(err) });
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="border rounded-lg p-4 flex items-start gap-4">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <h3 className="text-base font-semibold truncate">{provider.name}</h3>
          {!provider.has_api_key && (
            <span className="text-xs px-2 py-0.5 bg-yellow-100 text-yellow-800 rounded">
              No API key
            </span>
          )}
        </div>
        <p className="text-sm text-gray-500 truncate font-mono mt-0.5">{provider.base_url}</p>
        <div className="flex flex-wrap gap-1 mt-2">
          {provider.models.map((m) => (
            <span
              key={m.id}
              className="text-xs px-2 py-0.5 bg-gray-100 text-gray-700 rounded font-mono"
            >
              {m.id}
            </span>
          ))}
        </div>
        <p className="text-xs text-gray-400 mt-2 font-mono">
          Auth: {provider.auth_header_name}: {provider.auth_header_value_prefix}
          &lt;key&gt;
        </p>
      </div>

      <div className="flex flex-col items-end gap-2 shrink-0">
        <div className="flex gap-2">
          <button
            onClick={handleTest}
            disabled={testing || !provider.has_api_key}
            className="text-sm px-3 py-1 border rounded hover:bg-gray-50 disabled:opacity-50"
          >
            {testing ? "Testing..." : "Test"}
          </button>
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

        {testResult && (
          <div
            className={`text-xs px-2 py-1 rounded mt-1 ${
              testResult.success
                ? "bg-green-100 text-green-800"
                : "bg-red-100 text-red-800"
            }`}
          >
            {testResult.success
              ? "✓ Connection OK"
              : `✗ ${testResult.error ?? "Connection failed"}`}
          </div>
        )}
      </div>
    </div>
  );
}
