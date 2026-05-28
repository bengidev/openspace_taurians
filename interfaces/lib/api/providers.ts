import { invoke } from "@tauri-apps/api/core";
import type { Provider, ProviderCreate, ProviderTestResult, ProviderUpdate } from "../types/provider";

export async function providerList(): Promise<Provider[]> {
  return invoke("provider_list");
}

export async function providerGet(id: number): Promise<Provider | null> {
  return invoke("provider_get", { id });
}

export async function providerCreate(config: ProviderCreate): Promise<number> {
  return invoke("provider_create", { ...config });
}

export async function providerUpdate(config: ProviderUpdate): Promise<boolean> {
  return invoke("provider_update", { ...config });
}

export async function providerDelete(id: number): Promise<boolean> {
  return invoke("provider_delete", { id });
}

export async function providerTestConnection(providerId: number): Promise<ProviderTestResult> {
  return invoke("provider_test_connection", { providerId });
}
