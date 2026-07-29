/**
 * Provider state - Svelte 5 runes.
 */

import type {
  Provider,
  Model,
  AddProviderRequest,
  ProviderConfig,
  UUID,
  ProviderWithModels,
} from "../types";
import type { ModelWithProvider } from "../types/provider";
import * as providerApi from "../api/provider";
import * as modelApi from "../api/model";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";
import { isTauriEnvironment, getTauriEnvironmentInfo } from "../utils/tauri";

/** Broadcast the providers-updated event to all windows via Tauri 2's emit() API. */
async function emitProvidersUpdated(
  payload: Record<string, unknown>,
): Promise<void> {
  console.log("[emitProvidersUpdated] Checking environment...");
  console.log(
    "[emitProvidersUpdated] isTauriEnvironment:",
    isTauriEnvironment(),
  );

  if (!isTauriEnvironment()) {
    console.log(
      "[emitProvidersUpdated] Not in Tauri environment, skipping emit",
    );
    return;
  }

  try {
    console.log(
      "[emitProvidersUpdated] Emitting providers:updated event with payload:",
      payload,
    );
    // Tauri 2: emit() broadcasts to all windows automatically.
    await emit("providers:updated", payload);
    console.log(
      "[emitProvidersUpdated] Event emitted successfully to all windows",
    );
  } catch (error) {
    console.error(
      "[emitProvidersUpdated] Failed to broadcast providers:updated event:",
      error,
    );
  }
}

// Provider config templates (fetched from the backend).
export let providerConfigs = $state<{
  providers: ProviderConfig[];
  custom_providers: ProviderConfig[];
}>({
  providers: [],
  custom_providers: [],
});

export function getProviderConfig(
  providerType: string,
): ProviderConfig | undefined {
  return [
    ...providerConfigs.providers,
    ...providerConfigs.custom_providers,
  ].find((t) => t.provider_type === providerType);
}

// Whether a provider type is custom (openai-compatible / anthropic-compatible,
// etc.). Custom endpoints are not in the hand-ai catalog; models must be added
// manually.
export function isCustomProviderType(providerType: string): boolean {
  return providerConfigs.custom_providers.some(
    (t) => t.provider_type === providerType,
  );
}

// Provider icons come from models.dev remote SVGs keyed by provider_type (no
// local icons). models.dev returns a valid SVG for any slug — a real logo for
// known providers, a generic placeholder for unknown ones — so no 404 /
// broken-image handling is needed.
export function providerLogoUrl(
  providerType: string | undefined,
): string | undefined {
  if (!providerType) return undefined;
  return `https://models.dev/logos/${providerType}.svg`;
}

export function getProviderIcon(provider: Provider): string | undefined {
  return providerLogoUrl(provider.provider_type);
}

export function getProviderConfigById(
  providerId: string,
): ProviderConfig | undefined {
  const provider =
    providerState.providers.find((p) => p.id === providerId) ||
    providerState.providersWithModels.find((p) => p.id === providerId);

  if (provider) {
    return getProviderConfig(provider.provider_type);
  }

  return undefined;
}

export function getProviderIconById(providerId: string): string | undefined {
  const provider =
    providerState.providers.find((p) => p.id === providerId) ||
    providerState.providersWithModels.find((p) => p.id === providerId);
  return providerLogoUrl(provider?.provider_type);
}

export const providerState = $state({
  providers: [] as Provider[],

  // Selected provider for the detail page.
  currentProvider: null as Provider | null,

  // Provider being edited in the modal.
  editingProvider: null as Provider | null,

  // Models of the current provider.
  currentModels: [] as Model[],

  // Providers with their models (for chat features).
  providersWithModels: [] as ProviderWithModels[],
  providersWithModelsNeedRefresh: true,

  isLoading: false,
  isLoadingWithModels: false,

  // Provider id whose model list is being fetched, or null.
  isFetchingModels: null as UUID | null,

  error: null as string | null,
});

function markProvidersWithModelsDirty(
  reason: string,
  data?: Record<string, unknown>,
): void {
  console.log(
    "[markProvidersWithModelsDirty] Called with reason:",
    reason,
    "data:",
    data,
  );
  providerState.providersWithModelsNeedRefresh = true;
  const payload = data ? { reason, ...data } : { reason };
  console.log(
    "[markProvidersWithModelsDirty] Calling emitProvidersUpdated with payload:",
    payload,
  );
  void emitProvidersUpdated(payload);
}

let providersUpdatedUnlisten: UnlistenFn | null = null;

export function getEnabledProviders(): Provider[] {
  return providerState.providers.filter((p) => p.enabled);
}

export function getAllModels(): ModelWithProvider[] {
  return providerState.providersWithModels.flatMap((provider) =>
    provider.models.map((model) => ({
      ...model,
      providerName: provider.name,
      providerType: provider.provider_type,
    })),
  );
}

export function getFavoriteModels(): ModelWithProvider[] {
  return getAllModels().filter((model) => model.favorite);
}

export function getProviderDropdownOptions() {
  const preProviderOptions = providerConfigs.providers.map((provider) => ({
    value: provider.provider_type,
    label: provider.type_name,
    icon: providerLogoUrl(provider.provider_type),
  }));

  const customProviderOptions = providerConfigs.custom_providers.map(
    (provider) => ({
      value: provider.provider_type,
      label: provider.type_name,
      icon: providerLogoUrl(provider.provider_type),
    }),
  );

  return [
    {
      title: "",
      options: preProviderOptions,
    },
    {
      title: "",
      options: customProviderOptions,
    },
  ];
}

export const providerStateActions = {
  setCurrentProvider(provider: Provider | null): void {
    providerState.currentProvider = provider;
  },

  setCurrentProviderById(providerId: UUID): Provider | null {
    const provider = providerState.providers.find((p) => p.id === providerId);
    providerState.currentProvider = provider || null;
    return provider || null;
  },

  startEditProvider(provider: Provider | null): void {
    providerState.editingProvider = provider;
  },

  endEditProvider(): void {
    providerState.editingProvider = null;
  },

  updateCurrentProvider(updatedProvider: Provider): void {
    if (
      providerState.currentProvider &&
      providerState.currentProvider.id === updatedProvider.id
    ) {
      providerState.currentProvider = updatedProvider;
    }
  },

  async refreshCurrentProvider(): Promise<void> {
    if (providerState.currentProvider && providerState.currentProvider.id) {
      const providerId = providerState.currentProvider.id;
      try {
        const updatedProvider = await providerActions.getProvider(providerId);
        providerState.currentProvider = updatedProvider;

        await providerActions.fetchProviderModels(providerId, true);
      } catch (error) {
        console.error("Failed to refresh current provider:", error);
      }
    }
  },

  clearSelection(): void {
    providerState.currentProvider = null;
    providerState.editingProvider = null;
  },
};

export const providerActions = {
  async loadProviderConfigs(): Promise<void> {
    try {
      const configs = await providerApi.getProviderConfigs();
      providerConfigs.providers = configs.providers;
      providerConfigs.custom_providers = configs.custom_providers;
    } catch (error) {
      console.error("Failed to load provider templates:", error);
      // Do not rethrow — this must not block app startup.
    }
  },

  async loadProviders(): Promise<void> {
    try {
      providerState.isLoading = true;
      const providerList = await providerApi.getProviders();
      providerState.providers = providerList;
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "加载供应商列表失败";
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * Load providers with their models.
   * @param refreshFromRemote when true, pull the latest models from the remote
   *   and sync the database first; by default read from the local database only.
   */
  async loadProvidersWithModels(refreshFromRemote = false): Promise<void> {
    try {
      providerState.isLoadingWithModels = true;
      providerState.error = null;

      const providersWithModels =
        await modelApi.getAllModelsWithProviders(refreshFromRemote);
      providerState.providersWithModels = providersWithModels;

      console.log(
        "action do ->> providerState.providersWithModelsNeedRefresh: " +
          providerState.providersWithModelsNeedRefresh,
      );

      providerState.providersWithModelsNeedRefresh = false;
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "加载供应商列表失败";
      providerState.providersWithModelsNeedRefresh = true;
      throw error;
    } finally {
      providerState.isLoadingWithModels = false;
    }
  },

  async getProvider(providerId: string): Promise<Provider> {
    const response = await providerApi.getProvider(providerId);
    return response;
  },

  async createProvider(config: AddProviderRequest): Promise<Provider> {
    try {
      providerState.isLoading = true;
      const provider = await providerApi.createProvider(config);

      providerState.providers.push(provider);
      markProvidersWithModelsDirty("provider-created", {
        providerId: provider.id,
      });

      return provider;
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "创建供应商失败";
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  async updateProvider(
    providerId: UUID,
    config: Partial<AddProviderRequest>,
  ): Promise<void> {
    try {
      providerState.isLoading = true;
      const updatedProvider = await providerApi.updateProvider(
        providerId,
        config,
      );

      const index = providerState.providers.findIndex(
        (p) => p.id === providerId,
      );
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }
      markProvidersWithModelsDirty("provider-updated", { providerId });
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "更新供应商失败";
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  async deleteProvider(providerId: UUID): Promise<void> {
    try {
      providerState.isLoading = true;
      await providerApi.deleteProvider(providerId);

      providerState.providers = providerState.providers.filter(
        (p) => p.id !== providerId,
      );

      if (providerState.currentProvider?.id === providerId) {
        providerStateActions.clearSelection();
      }
      markProvidersWithModelsDirty("provider-deleted", { providerId });
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "删除供应商失败";
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  async fetchProviderModels(
    providerId: UUID,
    refreshFromRemote = false,
  ): Promise<void> {
    try {
      providerState.isFetchingModels = providerId;
      const models = await modelApi.getProviderModels(
        providerId,
        refreshFromRemote,
      );

      providerState.currentModels = models;

      const providersWithModelsIndex =
        providerState.providersWithModels.findIndex(
          (provider) => provider.id === providerId,
        );
      if (providersWithModelsIndex !== -1) {
        providerState.providersWithModels[providersWithModelsIndex] = {
          ...providerState.providersWithModels[providersWithModelsIndex],
          models,
        };
      }

      // Intentionally no needsRefresh flag here: after a remote refresh the
      // data is already current.
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "获取模型列表失败";
      throw error;
    } finally {
      providerState.isFetchingModels = null;
    }
  },

  /** Manually add a model to a custom provider and refresh its model list. */
  async addModel(
    providerId: UUID,
    modelId: string,
    name?: string,
  ): Promise<void> {
    try {
      await modelApi.addModel(providerId, modelId, name);
      await providerActions.fetchProviderModels(providerId, false);
      // Mark the providers-with-models cache dirty so the chat model picker
      // sees the new model.
      markProvidersWithModelsDirty("model-added", { providerId, modelId });
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "添加模型失败";
      throw error;
    }
  },

  async toggleProvider(providerId: UUID, enabled: boolean): Promise<void> {
    try {
      const updatedProvider = await providerApi.toggleProvider(
        providerId,
        enabled,
      );

      const index = providerState.providers.findIndex(
        (p) => p.id === providerId,
      );
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }

      const providersWithModelsIndex =
        providerState.providersWithModels.findIndex(
          (provider) => provider.id === providerId,
        );
      if (providersWithModelsIndex !== -1) {
        providerState.providersWithModels[providersWithModelsIndex] = {
          ...providerState.providersWithModels[providersWithModelsIndex],
          enabled,
        };
      }

      markProvidersWithModelsDirty("provider-toggled", { providerId, enabled });

      console.log(
        "set providerState.providersWithModelsNeedRefresh: " +
          providerState.providersWithModelsNeedRefresh,
      );
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "切换供应商状态失败";
      throw error;
    }
  },

  async toggleModel(
    providerId: UUID,
    modelId: string,
    enabled: boolean,
  ): Promise<void> {
    try {
      await modelApi.toggleModel(providerId, modelId, enabled);

      const index = providerState.currentModels.findIndex(
        (m) => m.id === modelId,
      );
      if (index !== -1) {
        providerState.currentModels[index] = {
          ...providerState.currentModels[index],
          enabled,
        };
      }

      const providerIndex = providerState.providersWithModels.findIndex(
        (p) => p.id === providerId,
      );
      if (providerIndex !== -1) {
        const modelIndex = providerState.providersWithModels[
          providerIndex
        ].models.findIndex((m) => m.id === modelId);
        if (modelIndex !== -1) {
          providerState.providersWithModels[providerIndex].models[modelIndex] =
            {
              ...providerState.providersWithModels[providerIndex].models[
                modelIndex
              ],
              enabled,
            };
        }
      }

      markProvidersWithModelsDirty("model-toggled", {
        providerId,
        modelId,
        enabled,
      });
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "切换模型状态失败";
      throw error;
    }
  },

  async toggleModelFavorite(
    providerId: UUID,
    modelId: string,
    favorite: boolean,
    options?: { skipRefreshFlag?: boolean },
  ): Promise<void> {
    try {
      await modelApi.toggleModelFavorite(providerId, modelId, favorite);

      const currentIndex = providerState.currentModels.findIndex(
        (m) => m.id === modelId,
      );
      if (currentIndex !== -1) {
        providerState.currentModels[currentIndex] = {
          ...providerState.currentModels[currentIndex],
          favorite,
        };
      }

      const providerIndex = providerState.providersWithModels.findIndex(
        (p) => p.id === providerId,
      );
      if (providerIndex !== -1) {
        const modelIndex = providerState.providersWithModels[
          providerIndex
        ].models.findIndex((m) => m.id === modelId);
        if (modelIndex !== -1) {
          providerState.providersWithModels[providerIndex].models[modelIndex] =
            {
              ...providerState.providersWithModels[providerIndex].models[
                modelIndex
              ],
              favorite,
            };
        }
      }

      if (!options?.skipRefreshFlag) {
        markProvidersWithModelsDirty("model-favorite-toggled", {
          providerId,
          modelId,
          favorite,
        });
      }
    } catch (error) {
      providerState.error =
        error instanceof Error ? error.message : "切换模型收藏状态失败";
      throw error;
    }
  },

  findModel(modelId: string): Model | undefined {
    return providerState.currentModels.find((m) => m.id === modelId);
  },

  clearError(): void {
    providerState.error = null;
  },

  reset(): void {
    providerState.providers = [];
    providerState.currentProvider = null;
    providerState.editingProvider = null;
    providerState.currentModels = [];
    providerState.providersWithModels = [];
    providerState.isLoading = false;
    providerState.isLoadingWithModels = false;
    providerState.isFetchingModels = null;
    providerState.error = null;
    providerState.providersWithModelsNeedRefresh = true;

    providerConfigs.providers = [];
    providerConfigs.custom_providers = [];
  },
};

/**
 * Register the providers:updated listener. Call from a component's onMount so
 * the Tauri environment is ready.
 */
export async function setupProvidersUpdatedListener(): Promise<void> {
  console.log("[setupProvidersUpdatedListener] Setting up listener...");
  console.log(
    "[setupProvidersUpdatedListener] Environment check:",
    getTauriEnvironmentInfo(),
  );
  console.log(
    "[setupProvidersUpdatedListener] providersUpdatedUnlisten:",
    providersUpdatedUnlisten,
  );

  if (!isTauriEnvironment()) {
    console.warn(
      "[setupProvidersUpdatedListener] ⚠️  Not in Tauri environment!",
    );
    console.warn(
      '  Make sure you are running "npm run tauri dev", not just "npm run dev"',
    );
    console.warn("  Cross-window events will not work in browser-only mode");
    return;
  }

  if (providersUpdatedUnlisten) {
    console.log("[setupProvidersUpdatedListener] Listener already set up");
    return;
  }

  try {
    console.log(
      "[setupProvidersUpdatedListener] Registering listener for providers:updated event",
    );
    providersUpdatedUnlisten = await listen("providers:updated", (event) => {
      console.log(
        "[providersUpdatedListener] providers:updated event received",
        event,
      );
      // Only mark for refresh, no auto-load; components check and load on
      // open as they need.
      providerState.providersWithModelsNeedRefresh = true;
      console.log(
        "[providersUpdatedListener] Set providersWithModelsNeedRefresh to true",
      );
    });
    console.log(
      "[setupProvidersUpdatedListener] Listener registered successfully",
    );
  } catch (error) {
    console.error(
      "[setupProvidersUpdatedListener] Failed to register providers:updated listener:",
      error,
    );
  }
}

export function cleanupProvidersUpdatedListener(): void {
  if (providersUpdatedUnlisten) {
    console.log("[cleanupProvidersUpdatedListener] Cleaning up listener");
    providersUpdatedUnlisten();
    providersUpdatedUnlisten = null;
  }
}
