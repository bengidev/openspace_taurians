"use client";

import { useState } from "react";
import type { Provider, ProviderCreate, ProviderUpdate, ModelInfo } from "@/lib/types/provider";

interface Props {
  provider?: Provider;
  onSubmit: (config: ProviderCreate | ProviderUpdate) => Promise<void>;
  onCancel: () => void;
}

const DEFAULT_OPENAI_TEMPLATE = {
  model: "{model}",
  messages: "{messages}",
  stream: "{stream}",
  temperature: "{temperature}",
};

export function ProviderForm({ provider, onSubmit, onCancel }: Props) {
  const [name, setName] = useState(provider?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(provider?.base_url ?? "");
  const [apiKey, setApiKey] = useState("");
  const [authHeaderName, setAuthHeaderName] = useState(provider?.auth_header_name ?? "Authorization");
  const [authHeaderPrefix, setAuthHeaderPrefix] = useState(provider?.auth_header_value_prefix ?? "Bearer ");
  const [responsePath, setResponsePath] = useState(provider?.response_path ?? "choices[0].message.content");
  const [models, setModels] = useState<ModelInfo[]>(provider?.models ?? []);
  const [templateStr, setTemplateStr] = useState(
    provider ? JSON.stringify(provider.request_body_template, null, 2) : JSON.stringify(DEFAULT_OPENAI_TEMPLATE, null, 2)
  );
  const [submitting, setSubmitting] = useState(false);

  // Model editor
  const [newModelId, setNewModelId] = useState("");
  const [newModelName, setNewModelName] = useState("");
  const [newModelCtx, setNewModelCtx] = useState("4096");

  function addModel() {
    if (!newModelId || !newModelName) return;
    setModels((prev) => [
      ...prev,
      { id: newModelId, name: newModelName, context_window: parseInt(newModelCtx) || 4096 },
    ]);
    setNewModelId("");
    setNewModelName("");
    setNewModelCtx("4096");
  }

  function removeModel(idx: number) {
    setModels((prev) => prev.filter((_, i) => i !== idx));
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    try {
      let template: Record<string, unknown>;
      try {
        template = JSON.parse(templateStr);
      } catch {
        alert("Request body template must be valid JSON");
        return;
      }

      if (provider) {
        await onSubmit({
          id: provider.id,
          name,
          base_url: baseUrl,
          ...(apiKey ? { api_key: apiKey } : {}),
          auth_header_name: authHeaderName,
          auth_header_value_prefix: authHeaderPrefix,
          models,
          request_body_template: template,
          response_path: responsePath,
        } as ProviderUpdate);
      } else {
        await onSubmit({
          name,
          base_url: baseUrl,
          ...(apiKey ? { api_key: apiKey } : {}),
          auth_header_name: authHeaderName,
          auth_header_value_prefix: authHeaderPrefix,
          models,
          request_body_template: template,
          response_path: responsePath,
        } as ProviderCreate);
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="grid grid-cols-2 gap-4">
        <label className="block">
          <span className="text-sm font-medium">Provider Name</span>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            placeholder="My Custom Provider"
            className="mt-1 block w-full rounded border border-gray-300 px-3 py-2"
          />
        </label>
        <label className="block">
          <span className="text-sm font-medium">Base URL</span>
          <input
            type="url"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            required
            placeholder="https://api.openai.com/v1"
            className="mt-1 block w-full rounded border border-gray-300 px-3 py-2"
          />
        </label>
      </div>

      <label className="block">
        <span className="text-sm font-medium">
          API Key {provider ? "(leave blank to keep existing)" : ""}
        </span>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="sk-..."
          className="mt-1 block w-full rounded border border-gray-300 px-3 py-2 font-mono"
        />
      </label>

      <div className="grid grid-cols-2 gap-4">
        <label className="block">
          <span className="text-sm font-medium">Auth Header Name</span>
          <input
            type="text"
            value={authHeaderName}
            onChange={(e) => setAuthHeaderName(e.target.value)}
            className="mt-1 block w-full rounded border border-gray-300 px-3 py-2"
          />
        </label>
        <label className="block">
          <span className="text-sm font-medium">Auth Header Prefix</span>
          <input
            type="text"
            value={authHeaderPrefix}
            onChange={(e) => setAuthHeaderPrefix(e.target.value)}
            placeholder="Bearer "
            className="mt-1 block w-full rounded border border-gray-300 px-3 py-2"
          />
        </label>
      </div>

      {/* Models section */}
      <fieldset className="border rounded p-3 space-y-3">
        <legend className="text-sm font-semibold">Models</legend>

        {models.length > 0 && (
          <ul className="space-y-1">
            {models.map((m, i) => (
              <li key={i} className="flex items-center gap-2 text-sm">
                <span className="font-mono">{m.id}</span>
                <span className="text-gray-500">({m.name})</span>
                <span className="text-gray-400">— {m.context_window.toLocaleString()} tokens</span>
                <button
                  type="button"
                  onClick={() => removeModel(i)}
                  className="ml-auto text-red-500 hover:underline"
                >
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className="grid grid-cols-3 gap-2">
          <input
            type="text"
            value={newModelId}
            onChange={(e) => setNewModelId(e.target.value)}
            placeholder="Model ID"
            className="rounded border border-gray-300 px-2 py-1 text-sm"
          />
          <input
            type="text"
            value={newModelName}
            onChange={(e) => setNewModelName(e.target.value)}
            placeholder="Display name"
            className="rounded border border-gray-300 px-2 py-1 text-sm"
          />
          <div className="flex gap-2">
            <input
              type="number"
              value={newModelCtx}
              onChange={(e) => setNewModelCtx(e.target.value)}
              placeholder="Context window"
              className="rounded border border-gray-300 px-2 py-1 text-sm flex-1"
            />
            <button
              type="button"
              onClick={addModel}
              className="px-3 py-1 bg-gray-200 rounded text-sm hover:bg-gray-300"
            >
              Add
            </button>
          </div>
        </div>
      </fieldset>

      {/* Template + response path */}
      <label className="block">
        <span className="text-sm font-medium">Request Body Template (JSON)</span>
        <textarea
          value={templateStr}
          onChange={(e) => setTemplateStr(e.target.value)}
          rows={6}
          className="mt-1 block w-full rounded border border-gray-300 px-3 py-2 font-mono text-sm"
        />
        <span className="text-xs text-gray-500">
          Placeholders: {"{model}"}, {"{messages}"}, {"{stream}"}, {"{temperature}"}
        </span>
      </label>

      <label className="block">
        <span className="text-sm font-medium">Response Path</span>
        <input
          type="text"
          value={responsePath}
          onChange={(e) => setResponsePath(e.target.value)}
          required
          placeholder="choices[0].message.content"
          className="mt-1 block w-full rounded border border-gray-300 px-3 py-2 font-mono"
        />
        <span className="text-xs text-gray-500">
          Dot-separated path to extract content from the API response
        </span>
      </label>

      <div className="flex gap-3 pt-2">
        <button
          type="submit"
          disabled={submitting}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
        >
          {submitting ? "Saving..." : provider ? "Save Changes" : "Create Provider"}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="px-4 py-2 border rounded hover:bg-gray-100"
        >
          Cancel
        </button>
      </div>
    </form>
  );
}
