/**
 * 供应商相关类型定义
 */

import type { BaseEntity } from './index';

// 供应商类型
export type ProviderType = 'openai' | 'anthropic' | 'google' | 'deepseek' | 'openrouter' | 'custom-openai' | 'custom-anthropic';

// 供应商状态
export type ProviderStatus = 'active' | 'inactive' | 'error' | 'testing';

// 供应商配置
export interface Provider extends BaseEntity {
  name: string;
  type: ProviderType;
  baseUrl: string;
  status: ProviderStatus;
  enabled: boolean;
  models: Model[];
  lastProbeAt?: number;
  probeResult?: ProbeResult;
}

// 模型信息
export interface Model {
  id: string;
  name: string;
  provider: string;
  contextLength?: number;
  inputCost?: number;
  outputCost?: number;
  supportedFeatures: ModelFeature[];
  enabled: boolean;
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
  type: ProviderType;
  baseUrl: string;
  apiKey: string;
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