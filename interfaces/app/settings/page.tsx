"use client";

import { useEffect, useState } from "react";
import { ProviderList } from "@/components/settings/ProviderList";
import { ProviderForm } from "@/components/settings/ProviderForm";
import { providerList, providerCreate, providerUpdate, providerDelete } from "@/lib/api/providers";
import type { Provider, ProviderCreate, ProviderUpdate } from "@/lib/types/provider";

export default function SettingsPage() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadProviders();
  }, []);

  async function loadProviders() {
    try {
      const list = await providerList();
      setProviders(list);
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
      await loadProviders();
    } catch (err) {
      alert(`Failed to create provider: ${err}`);
      throw err;
    }
  }

  async function handleUpdate(config: ProviderUpdate) {
    if (!editingProvider) return;
    try {
      await providerUpdate(config);
      setEditingProvider(null);
      await loadProviders();
    } catch (err) {
      alert(`Failed to update provider: ${err}`);
      throw err;
    }
  }

  async function handleDelete(id: number) {
    if (!confirm("Delete this provider?")) return;
    try {
      await providerDelete(id);
      await loadProviders();
    } catch (err) {
      alert(`Failed to delete provider: ${err}`);
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

      <ProviderList
        providers={providers}
        onEdit={handleEdit}
        onDelete={handleDelete}
      />
    </div>
  );
}
