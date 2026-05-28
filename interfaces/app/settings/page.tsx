"use client";

import { useEffect, useState } from "react";
import { ProviderList } from "@/components/settings/ProviderList";
import { ProviderForm } from "@/components/settings/ProviderForm";
import {
  providerList,
  providerCreate,
  providerUpdate,
  providerDelete,
  activeProviderGet,
  activeProviderSet,
  activeProviderClear,
} from "@/lib/api/providers";
import type { ActiveProvider, Provider, ProviderCreate, ProviderUpdate } from "@/lib/types/provider";

export default function SettingsPage() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [active, setActive] = useState<ActiveProvider | null>(null);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    try {
      const [list, currentActive] = await Promise.all([
        providerList(),
        activeProviderGet(),
      ]);
      setProviders(list);
      setActive(currentActive);
    } catch (err) {
      console.error("Failed to load providers:", err);
    } finally {
      setLoading(false);
    }
  }

  async function handleCreate(config: ProviderCreate) {
    try {
      await providerCreate(config);
      setShowForm(false);
      await loadData();
    } catch (err) {
      alert(`Failed to create provider: ${err}`);
      throw err;
    }
  }

  async function handleUpdate(config: ProviderCreate | ProviderUpdate) {
    if (!editingProvider) return;
    try {
      await providerUpdate(config as ProviderUpdate);
      setEditingProvider(null);
      await loadData();
    } catch (err) {
      alert(`Failed to update provider: ${err}`);
      throw err;
    }
  }

  async function handleDelete(id: number) {
    if (!confirm("Delete this provider?")) return;
    try {
      await providerDelete(id);
      if (active?.provider_id === id) {
        setActive(null);
      }
      await loadData();
    } catch (err) {
      alert(`Failed to delete provider: ${err}`);
    }
  }

  async function handleSetActive(providerId: number, model: string) {
    try {
      await activeProviderSet(providerId, model);
      setActive({ provider_id: providerId, model });
    } catch (err) {
      alert(`Failed to set active provider: ${err}`);
    }
  }

  async function handleClearActive() {
    if (!confirm("Clear the active provider? Downstream AI features will be unconfigured.")) return;
    try {
      await activeProviderClear();
      setActive(null);
    } catch (err) {
      alert(`Failed to clear active provider: ${err}`);
    }
  }

  function handleEdit(provider: Provider) {
    setEditingProvider(provider);
  }

  function handleCancelEdit() {
    setEditingProvider(null);
  }

  if (loading) {
    return (
      <div className="p-6">
        <p className="text-sm text-gray-500">Loading...</p>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">Providers</h1>
        <button
          onClick={() => setShowForm(true)}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700"
        >
          Add Provider
        </button>
      </div>

      {showForm && (
        <div className="border rounded-lg p-4">
          <h2 className="text-lg font-semibold mb-4">New Provider</h2>
          <ProviderForm
            onSubmit={handleCreate}
            onCancel={() => setShowForm(false)}
          />
        </div>
      )}

      {editingProvider && (
        <div className="border rounded-lg p-4">
          <h2 className="text-lg font-semibold mb-4">Edit Provider</h2>
          <ProviderForm
            provider={editingProvider}
            onSubmit={handleUpdate}
            onCancel={handleCancelEdit}
          />
        </div>
      )}

      {active ? (
        <div className="border border-blue-300 rounded-lg p-4 bg-blue-50/30">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-sm font-semibold text-blue-900">Active Provider</h2>
              <p className="text-sm text-blue-800 font-mono mt-1">
                {providers.find((p) => p.id === active.provider_id)?.name ?? "Unknown"} —{" "}
                {active.model}
              </p>
            </div>
            <button
              onClick={handleClearActive}
              className="text-sm px-3 py-1 border border-blue-400 text-blue-700 rounded hover:bg-blue-100"
            >
              Clear
            </button>
          </div>
        </div>
      ) : (
        <div className="border border-dashed rounded-lg p-4 bg-gray-50 text-center">
          <p className="text-sm text-gray-500">
            No active provider selected. AI features are unconfigured. Choose a provider and model below to activate.
          </p>
        </div>
      )}

      <ProviderList
        providers={providers}
        active={active}
        onEdit={handleEdit}
        onDelete={handleDelete}
        onSetActive={handleSetActive}
      />
    </div>
  );
}
