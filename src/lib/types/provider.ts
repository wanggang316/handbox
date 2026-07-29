import type { BaseEntity } from "./index";

export interface Provider extends BaseEntity {
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  enabled: boolean;
}

export interface ProviderWithModels extends Provider {
  models: Model[];
}

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

export type ParameterLevel = "base" | "advance";

export type ParameterComponent =
  | "slider"
  | "switch"
  | "responses_reasoning"
  | "completions_reasoning"
  | "thinking"
  | "openrouter_reasoning";

export interface SliderProps {
  default?: number | null;
  min?: number | null;
  max?: number | null;
  step?: number | null;
  name: string;
  show_toggle?: boolean | null;
  tips?: string | null;
}

export interface SwitchProps {
  default?: boolean | null;
  name: string;
  tips?: string | null;
}

export interface ResponsesReasoningProps {
  name?: string | null;
  effort_options?: Record<string, string[]> | null;
  summary_options?: Record<string, string[]> | null;
  tips?: string | null;
}

export interface CompletionsReasoningProps {
  name?: string | null;
  include_reasoning?: boolean | null;
  effort_options?: Record<string, string[]> | null;
  tips?: string | null;
}

// Backward-compatible alias.
export type ReasoningProps = ResponsesReasoningProps;

export interface BudgetOptions {
  dynamic?: number | null; // -1 = dynamic.
  disable?: number | null; // 0 = disabled.
  range?: [number, number] | null; // [min, max] slider range.
}

export interface BudgetConfig {
  models: string[]; // Format: "provider_type/model_id".
  options: BudgetOptions;
  default: string; // "dynamic" | "disable" | "range"
}

export interface ThinkingProps {
  name?: string | null;
  budget_configs?: BudgetConfig[] | null;
  tips?: string | null;
  include_thoughts_tip?: string | null;
  budget_tip?: string | null;
}

export interface OpenrouterReasoningProps {
  name: string;
  tips?: string | null;
  effect_tips?: string | null;
  max_tokens_tips?: string | null;
  props?: string[] | null;
  effort_options?: string[] | null;
  max_tokens?: [number, number] | null;
}

export type ComponentProps =
  | SliderProps
  | SwitchProps
  | ResponsesReasoningProps
  | CompletionsReasoningProps
  | ThinkingProps
  | OpenrouterReasoningProps;

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
  support_tools: boolean;
  support_image: boolean;
  enabled: boolean;
  favorite: boolean;
  created_at: number;
  updated_at: number;
}

export interface ModelWithProvider extends Model {
  providerName: string;
  providerType: string;
}

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

export interface AddProviderRequest {
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
  enabled?: boolean;
}

export interface ProviderConfig {
  provider_type: string;
  type_name: string;
  default_name: string;
  default_base_url: string;
  icon: string;
  description?: string;
}

export interface ProviderConfigsResponse {
  providers: ProviderConfig[];
  custom_providers: ProviderConfig[];
}

export interface ListModelsRequest {
  providerId: string;
  refreshFromRemote?: boolean;
}

export interface ToggleModelFavoriteRequest {
  provider_id: string;
  model_id: string;
  favorite: boolean;
}
