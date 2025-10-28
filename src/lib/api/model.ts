/**
 * 模型相关 API 封装
 */

import { apiCall } from './index';
import type {
	Model,
	ToggleModelFavoriteRequest,
	ProviderWithModels,
	UUID
} from '../types';

/**
 * 获取供应商模型列表
 */
export async function getProviderModels(
	providerId: UUID,
	refreshFromRemote: boolean
): Promise<Model[]> {
	return apiCall<Model[]>('model_list_by_provider', {
		request: {
			provider_id: providerId,
			refresh_from_remote: refreshFromRemote
		}
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
	return apiCall<void>('model_toggle', {
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
	return apiCall<void>('model_toggle_favorite', {
		request: {
			provider_id: providerId,
			model_id: modelId,
			favorite
		}
	});
}

/**
 * 获取所有供应商及其模型（包含收藏状态）
 */
export async function getAllModelsWithProviders(
	refreshFromRemote: boolean = false
): Promise<ProviderWithModels[]> {
	return apiCall<ProviderWithModels[]>('provider_list_with_models', {
		refresh_from_remote: refreshFromRemote
	});
}

/**
 * 获取所有可用模型（所有启用供应商的启用模型）
 */
export async function getAvailableModels(): Promise<Model[]> {
	return apiCall<Model[]>('model_get_available');
}

/**
 * 统计使用指定模型的聊天数量
 */
export async function countChatsUsingModel(modelId: string): Promise<number> {
	return apiCall<number>('model_count_chats', { modelId });
}
