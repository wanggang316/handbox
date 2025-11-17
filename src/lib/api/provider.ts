/**
 * 供应商相关 API 封装
 */

import { apiCall } from './index';
import type {
	Provider,
	AddProviderRequest,
	ProviderConfig,
	ProviderConfigsResponse,
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
export async function createProvider(config: AddProviderRequest): Promise<Provider> {
  return apiCall<Provider>('provider_create', { config });
}

/**
 * 更新供应商配置
 */
export async function updateProvider(
  providerId: UUID,
  config: Partial<AddProviderRequest>
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
export async function toggleProvider(providerId: UUID, enabled: boolean): Promise<Provider> {
	return apiCall<Provider>('provider_toggle', {
		request: {
			provider_id: providerId,
			enabled
		}
	});
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
export async function getProviderConfigByType(
	providerType: string
): Promise<ProviderConfig | null> {
	return apiCall<ProviderConfig | null>('get_provider_config_by_type', {
		provider_type: providerType
	});
}

/**
 * 统计使用指定供应商的聊天数量
 */
export async function countChatsUsingProvider(providerId: string): Promise<number> {
	return apiCall<number>('provider_count_chats', { providerId });
}