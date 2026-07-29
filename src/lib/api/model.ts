import { apiCall } from "./index";
import type {
  Model,
  ToggleModelFavoriteRequest,
  ProviderWithModels,
  UUID,
} from "../types";

export async function getProviderModels(
  providerId: UUID,
  refreshFromRemote: boolean,
): Promise<Model[]> {
  return apiCall<Model[]>("model_list_by_provider", {
    request: {
      provider_id: providerId,
      refresh_from_remote: refreshFromRemote,
    },
  });
}

export async function toggleModel(
  providerId: UUID,
  modelId: string,
  enabled: boolean,
): Promise<void> {
  return apiCall<void>("model_toggle", {
    request: {
      provider_id: providerId,
      model_id: modelId,
      enabled,
    },
  });
}

export async function toggleModelFavorite(
  providerId: UUID,
  modelId: string,
  favorite: boolean,
): Promise<void> {
  return apiCall<void>("model_toggle_favorite", {
    request: {
      provider_id: providerId,
      model_id: modelId,
      favorite,
    },
  });
}

export async function getAllModelsWithProviders(
  refreshFromRemote: boolean = false,
): Promise<ProviderWithModels[]> {
  return apiCall<ProviderWithModels[]>("provider_list_with_models", {
    refresh_from_remote: refreshFromRemote,
  });
}

/** Enabled models across all enabled providers. */
export async function getAvailableModels(): Promise<Model[]> {
  return apiCall<Model[]>("model_get_available");
}

/**
 * Custom endpoints (openai-/anthropic-compatible) are not in the hand-ai
 * catalog and cannot auto-sync, so users add model ids by hand. Custom
 * providers only.
 */
export async function addModel(
  providerId: UUID,
  modelId: string,
  name?: string,
): Promise<Model> {
  return apiCall<Model>("model_add", { providerId, modelId, name });
}
