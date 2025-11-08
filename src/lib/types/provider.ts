/**
 * 供应商相关类型定义
 */

import type { BaseEntity } from "./index";

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

// 模型参数定义
export interface ModelParameter {
  name: string;
  default?: unknown;
  min?: unknown;
  max?: unknown;
}

export type ChatMethodName =
  | "completions"
  | "responses"
  | "google_generate_content";

// 参数显示等级
export type ParameterLevel = "base" | "advance";

// 参数组件类型
export type ParameterComponent = "slider" | "switch";

// 滑块组件属性
export interface SliderProps {
  default?: number | null;
  min?: number | null;
  max?: number | null;
  step?: number | null;
  name: string;
  show_toggle?: boolean | null;
}

// 开关组件属性
export interface SwitchProps {
  default?: boolean | null;
  name: string;
}

// 组件属性联合类型
export type ComponentProps = SliderProps | SwitchProps;

// 参数响应 (替换原 ChatMethodParameter)
export interface ModelParameterResponse {
  name: string;
  support: boolean;
  component: ParameterComponent;
  props: ComponentProps;
  level: ParameterLevel;
}

export interface ChatMethodResponse {
  name: ChatMethodName;
  parameters?: ModelParameterResponse[] | null;
}

// 模型信息
export interface Model {
  id: string;
  provider_id: string;
  name: string;
  context_length?: number;
  output_max_tokens?: number;
  display_context_length?: string;
  display_output_max_tokens?: string;
  supported_features?: ModelFeature[] | null;
  description?: string;
  input_modalities?: ModelModality[];
  output_modalities?: ModelModality[];
  metadata?: unknown;
  pricing?: ModelPricing;
  url?: string | null;
  parameters?: ModelParameter[];
  supported_parameters?: string[] | null;
  supported_chat_methods?: ChatMethodName[] | null;
  chat_method?: ChatMethodResponse | null;
  enabled: boolean;
  favorite: boolean;
  created_at: number;
  updated_at: number;
}

// 带供应商信息的模型（用于前端显示）
export interface ModelWithProvider extends Model {
  providerName: string;
  providerType: string;
}

// 模型特性
export type ModelFeature = string;

export type ModelModality =
  | "text"
  | "image"
  | "images"
  | "pdf"
  | "file"
  | "audio"
  | "video";

export interface ModelPricing {
  input_text?: string | null;
  output_text?: string | null;
}

// 添加供应商请求
export interface AddProviderRequest {
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  enabled?: boolean;
}

// 供应商配置选项（从后端获取）
export interface ProviderConfig {
  provider_type: string;
  type_name: string;
  default_name: string;
  default_base_url: string;
  icon: string;
  chat_api_type: string;
  model_api_type: string;
  description?: string;
}

// 供应商配置响应
export interface ProviderConfigsResponse {
  providers: ProviderConfig[];
  custom_providers: ProviderConfig[];
}

// 模型列表请求
export interface ListModelsRequest {
  providerId: string;
  refreshFromRemote?: boolean;
}

// 模型收藏切换请求
export interface ToggleModelFavoriteRequest {
  provider_id: string;
  model_id: string;
  favorite: boolean;
}
