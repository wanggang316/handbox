/**
 * 供应商相关 API 封装
 */

import { apiCall } from './index';
import type { 
  Provider, 
  ProviderConfig, 
  ListModelsRequest,
  ListModelsResponse,
  FrontendProviderConfig,
  ProviderConfigsResponse,
  ToggleModelFavoriteRequest,
  UUID 
} from '../types';

/**
 * 获取供应商列表
 */
export async function getProviders(): Promise<Provider[]> {
  return apiCall<Provider[]>('provider_list');
}

/**
 * 获取供应商详情
 */
export async function getProvider(providerId: UUID): Promise<Provider> {
  return apiCall<Provider>('provider_get', { providerId: providerId });
}

/**
 * 创建供应商
 */
export async function createProvider(config: ProviderConfig): Promise<Provider> {
  return apiCall<Provider>('provider_create', { config });
}

/**
 * 更新供应商配置
 */
export async function updateProvider(
  providerId: UUID,
  config: Partial<ProviderConfig>
): Promise<Provider> {
  return apiCall<Provider>('provider_update', { providerId: providerId, config });
}

/**
 * 删除供应商
 */
export async function deleteProvider(providerId: UUID): Promise<void> {
  return apiCall<void>('provider_delete', { providerId: providerId });
}

/**
 * 启用/禁用供应商
 */
export async function toggleProvider(
  providerId: UUID,
  enabled: boolean
): Promise<Provider> {
  return apiCall<Provider>('provider_toggle', { 
    request: {
      provider_id: providerId,
      enabled
    }
  });
}

/**
 * 获取供应商模型列表
 */
export async function getProviderModels(providerId: UUID, forceRefresh: boolean): Promise<ListModelsResponse> {
  return apiCall<ListModelsResponse>('provider_list_models', { request: {
    provider_id: providerId,
    force_refresh: forceRefresh
  }});
}

/**
 * 启用/禁用模型
 */
export async function toggleModel(
  providerId: UUID,
  modelId: string,
  enabled: boolean
): Promise<void> {
  return apiCall<void>('provider_toggle_model', { 
    request: {
      provider_id: providerId,
      model_id: modelId,
      enabled
    }
  });
}

/**
 * 切换模型收藏状态
 */
export async function toggleModelFavorite(
  providerId: UUID,
  modelId: string,
  favorite: boolean
): Promise<void> {
  return apiCall<void>('provider_toggle_model_favorite', { 
    request: {
      provider_id: providerId,
      model_id: modelId,
      favorite
    }
  });
}

/**
 * 获取所有可用模型
 */
export async function getAvailableModels(): Promise<Array<{ provider: Provider; models: any[] }>> {
  return apiCall<Array<{ provider: Provider; models: any[] }>>('provider_get_available_models');
}

/**
 * 获取供应商配置模板（用于添加供应商时的选择）
 */
export async function getProviderConfigs(): Promise<ProviderConfigsResponse> {
  return apiCall<ProviderConfigsResponse>('get_provider_configs');
}

/**
 * 根据类型获取供应商配置
 */
export async function getProviderConfigByType(providerType: string): Promise<FrontendProviderConfig | null> {
  return apiCall<FrontendProviderConfig | null>('get_provider_config_by_type', { provider_type: providerType });
}