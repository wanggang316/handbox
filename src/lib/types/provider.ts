/**
 * 供应商相关类型定义
 */

import type { BaseEntity } from './index';

// 供应商类型
export type ProviderType = 'openai' | 'anthropic' | 'google' | 'deepseek' | 'openrouter' | 'custom-openai' | 'custom-anthropic';

// 供应商状态
export type ProviderStatus = 'enabled' | 'disabled' | 'idle' | 'error';

// 供应商配置
export interface Provider extends BaseEntity {
  name: string;
  provider_type: ProviderType;
  base_url: string;
  api_key: string;
  status: ProviderStatus;
  enabled: boolean;
  last_probe_at?: number;
  probe_result?: ProbeResult;
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
  created_at: number;
  updated_at: number;
}

// 模型特性
export type ModelFeature = 'text' | 'vision' | 'function-calling' | 'streaming' | 'reasoning';

// 探活结果
export interface ProbeResult {
  success: boolean;
  latency?: number;
  error?: string;
  timestamp: number;
}

// 供应商配置请求
export interface ProviderConfig {
  name?: string;
  provider_type: ProviderType;
  base_url: string;
  api_key: string;
  enabled?: boolean;
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