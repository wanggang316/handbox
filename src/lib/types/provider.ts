/**
 * 供应商相关类型定义
 */

import type { BaseEntity } from './index';

// 供应商类型
// export type ProviderType = 'openai' | 'anthropic' | 'google' | 'deepseek' | 'openrouter' | 'custom-openai' | 'custom-anthropic';



// 供应商配置
export interface Provider extends BaseEntity {
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  enabled: boolean;
}

// 带模型的供应商
export interface ProviderWithModels extends Provider {
  models: Model[];
}

// 模型信息
export interface Model {
  id: string;
  provider_id: string;
  name: string;
  context_length?: number;
  input_cost?: number;
  output_cost?: number;
  supported_features: ModelFeature[];
  enabled: boolean;
  favorite: boolean;
  created_at: number;
  updated_at: number;
}

// 模型特性
export type ModelFeature = 'text' | 'vision' | 'function-calling' | 'streaming' | 'reasoning';



// 供应商配置请求
export interface ProviderConfig {
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  enabled?: boolean;
}

// 前端供应商配置选项（从后端获取）
export interface FrontendProviderConfig {
  provider_type: string;
  type_name: string;
  default_name: string;
  default_base_url: string;
  icon: string;
  chat_api_type: string;
  model_api_type: string;
  description?: string;
}

// 前端供应商配置响应
export interface ProviderConfigsResponse {
  providers: FrontendProviderConfig[];
  custom_providers: FrontendProviderConfig[];
}

// 模型列表请求
export interface ListModelsRequest {
  providerId: string;
  forceRefresh?: boolean;
}

// 模型列表响应
export interface ListModelsResponse {
  models: Model[];
  cached: boolean;
  timestamp: number;
}

// 模型收藏切换请求
export interface ToggleModelFavoriteRequest {
  provider_id: string;
  model_id: string;
  favorite: boolean;
}