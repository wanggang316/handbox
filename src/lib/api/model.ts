/**
 * 模型相关 API 封装
 */

import { apiCall } from './index';
import type {
	Model,
	ListModelsRequest,
	ListModelsResponse,
	ToggleModelFavoriteRequest,
	ProviderWithModels,
	UUID
} from '../types';

/**
 * 获取供应商模型列表
 */
export async function getProviderModels(
	providerId: UUID,
	forceRefresh: boolean
): Promise<ListModelsResponse> {
	return apiCall<ListModelsResponse>('model_list_by_provider', {
		request: {
			provider_id: providerId,
			force_refresh: forceRefresh
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
	forceRefresh: boolean = false
): Promise<ProviderWithModels[]> {
	return apiCall<ProviderWithModels[]>('model_get_all_with_providers', {
		force_refresh: forceRefresh
	});
}

/**
 * 获取所有收藏的模型
 */
export async function getFavoriteModels(): Promise<Model[]> {
	return apiCall<Model[]>('model_get_favorites');
}

/**
 * 获取所有可用模型（所有启用供应商的启用模型）
 */
export async function getAvailableModels(): Promise<Model[]> {
	return apiCall<Model[]>('model_get_available');
}
