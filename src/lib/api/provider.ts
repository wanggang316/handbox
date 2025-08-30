/**
 * 供应商相关 API 封装
 */

import { apiCall } from './index';
import type { 
  Provider, 
  ProviderConfig, 
  ProbeResult, 
  ListModelsRequest,
  ListModelsResponse,
  UUID 
} from '../types';

/**
 * 获取供应商列表
 */
export async function getProviders(): Promise<Provider[]> {
  return apiCall<Provider[]>('provider_list');
}

/**
 * 获取预定义供应商模板列表
 */
export async function getPredefinedProviders(): Promise<Provider[]> {
  return apiCall<Provider[]>('provider_list_predefined');
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
  return apiCall<Provider>('provider_create', config);
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
 * 探活检测供应商
 */
export async function probeProvider(providerId: UUID): Promise<ProbeResult> {
  return apiCall<ProbeResult>('provider_probe', { providerId: providerId });
}

/**
 * 获取供应商模型列表
 */
export async function getProviderModels(request: ListModelsRequest): Promise<ListModelsResponse> {
  return apiCall<ListModelsResponse>('provider_list_models', request);
}

/**
 * 启用/禁用供应商
 */
export async function toggleProvider(
  providerId: UUID,
  enabled: boolean
): Promise<Provider> {
  return apiCall<Provider>('provider_toggle', { 
    providerId, 
    enabled 
  });
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
    providerId, 
    modelId, 
    enabled 
  });
}

/**
 * 获取所有可用模型
 */
export async function getAvailableModels(): Promise<Array<{ provider: Provider; models: any[] }>> {
  return apiCall<Array<{ provider: Provider; models: any[] }>>('provider_get_available_models');
}