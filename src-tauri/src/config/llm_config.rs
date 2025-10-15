// LLM 配置管理器
// 从 llm_config.json 读取供应商和模型配置信息

use crate::storage::types::ModelFeature;
use handbox_llm::config::{
    LlmConfigProvider, LlmModelExtraInfo as LlmClientModelExtraInfo, LlmProviderConfig,
};
use handbox_llm::types::{LlmApiType, LlmModelApiType, LlmModelFeature, LlmModelModality};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

/// 模型额外信息配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExtraInfo {
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub input_cost_per_1k: Option<f32>,
    pub output_cost_per_1k: Option<f32>,
    pub features: Vec<String>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<String>>,
    pub output_modalities: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub pricing: Option<Value>,
    pub support_parameters: Option<Vec<String>>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
}

/// 供应商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub type_name: String,
    pub default_name: String,
    pub default_base_url: String,
    pub icon: String,
    pub chat_api_type: String,  // "openai" | "google" | "anthropic"
    pub model_api_type: String, // "openai" | "openai+local" | "google" | "anthropic" | "openrouter"
    pub model_local: Option<HashMap<String, ModelExtraInfo>>,
    pub support_parameters: Option<Vec<String>>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
}

/// LLM 配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
    pub support_parameters: Option<Vec<String>>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
}

impl LlmConfig {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            custom_providers: Vec::new(),
            support_parameters: None,
            default_parameters: None,
            max_parameters: None,
        }
    }

    /// 从配置文件加载
    pub fn load() -> Self {
        let mut config = Self::new();
        match config.load_file() {
            Ok(()) => {
                tracing::info!("Successfully loaded LLM config from llm_config.json");
            }
            Err(e) => {
                tracing::warn!("Failed to load config file: {}. Using empty config", e);
            }
        }
        config
    }

    /// 加载配置文件
    fn load_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string("llm_config.json")
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        let loaded_config: LlmConfig = serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;

        self.providers = loaded_config.providers;
        self.custom_providers = loaded_config.custom_providers;

        tracing::info!(
            "Loaded {} providers and {} custom providers from config",
            self.providers.len(),
            self.custom_providers.len()
        );

        Ok(())
    }

    /// 根据类型获取供应商配置
    pub fn get_provider_config(&self, provider_type: &str) -> Option<&ProviderConfig> {
        // 先在标准供应商中查找
        if let Some(config) = self
            .providers
            .iter()
            .find(|p| p.provider_type == provider_type)
        {
            return Some(config);
        }

        // 再在自定义供应商中查找
        self.custom_providers
            .iter()
            .find(|p| p.provider_type == provider_type)
    }

    /// 获取所有可用的供应商配置
    pub fn get_all_provider_configs(&self) -> Vec<&ProviderConfig> {
        let mut all_configs = Vec::new();
        all_configs.extend(self.providers.iter());
        all_configs.extend(self.custom_providers.iter());
        all_configs
    }

    /// 获取模型额外信息
    pub fn get_model_extra_info(
        &self,
        provider_type: &str,
        model_id: &str,
    ) -> Option<&ModelExtraInfo> {
        let config = self.get_provider_config(provider_type)?;

        // 优先从 model_local 中查找
        if let Some(local_models) = &config.model_local {
            if let Some(model_info) = local_models.get(model_id) {
                return Some(model_info);
            }
        }

        None
    }

    /// 转换特性字符串为 ModelFeature 枚举
    pub fn convert_features(&self, features: &[String]) -> Vec<ModelFeature> {
        features
            .iter()
            .filter_map(|f| match f.as_str() {
                "reasoning" => Some(ModelFeature::Reasoning),
                "tool" => Some(ModelFeature::Tool),
                _ => None,
            })
            .collect()
    }

    /// 获取模型支持的参数（级联：模型 -> 供应商 -> 全局）
    pub fn get_supported_parameters(&self, provider_type: &str, model_id: &str) -> Vec<String> {
        // 1. 先尝试从模型级别获取
        if let Some(model_info) = self.get_model_extra_info(provider_type, model_id) {
            if let Some(params) = &model_info.support_parameters {
                if !params.is_empty() {
                    return params.clone();
                }
            }
        }

        // 2. 从供应商级别获取
        if let Some(provider_config) = self.get_provider_config(provider_type) {
            if let Some(params) = &provider_config.support_parameters {
                if !params.is_empty() {
                    return params.clone();
                }
            }
        }

        // 3. 从全局配置获取
        if let Some(params) = &self.support_parameters {
            return params.clone();
        }

        // 4. 如果都没有，返回空数组
        Vec::new()
    }

    /// 获取参数默认值（级联：模型 -> 供应商 -> 全局）
    pub fn get_parameter_defaults(
        &self,
        provider_type: &str,
        model_id: &str,
    ) -> HashMap<String, Value> {
        let mut defaults = HashMap::new();

        // 1. 从全局配置获取默认值（最外层）
        if let Some(global_defaults) = &self.default_parameters {
            defaults.extend(global_defaults.clone());
        }

        // 2. 从供应商级别获取默认值（覆盖全局）
        if let Some(provider_config) = self.get_provider_config(provider_type) {
            if let Some(provider_defaults) = &provider_config.default_parameters {
                defaults.extend(provider_defaults.clone());
            }
        }

        // 3. 从模型级别获取默认值（覆盖供应商和全局）
        if let Some(model_info) = self.get_model_extra_info(provider_type, model_id) {
            if let Some(model_defaults) = &model_info.default_parameters {
                defaults.extend(model_defaults.clone());
            }
        }

        defaults
    }

    /// 获取参数最大值（级联：模型 -> 供应商 -> 全局）
    pub fn get_max_parameters(
        &self,
        provider_type: &str,
        model_id: &str,
    ) -> HashMap<String, Value> {
        let mut max_params = HashMap::new();

        if let Some(global_max) = &self.max_parameters {
            max_params.extend(global_max.clone());
        }

        if let Some(provider_config) = self.get_provider_config(provider_type) {
            if let Some(provider_max) = &provider_config.max_parameters {
                max_params.extend(provider_max.clone());
            }

            if let Some(model_local) = &provider_config.model_local {
                if let Some(model_info) = model_local.get(model_id) {
                    if let Some(model_max) = &model_info.max_parameters {
                        max_params.extend(model_max.clone());
                    }
                }
            }
        } else if let Some(model_info) = self.get_model_extra_info(provider_type, model_id) {
            if let Some(model_max) = &model_info.max_parameters {
                max_params.extend(model_max.clone());
            }
        }

        max_params
    }

    pub fn convert_modalities(&self, modalities: &[String]) -> Vec<LlmModelModality> {
        modalities
            .iter()
            .filter_map(|m| m.parse::<LlmModelModality>().ok())
            .collect()
    }
}

fn map_model_feature(feature: ModelFeature) -> LlmModelFeature {
    match feature {
        ModelFeature::Reasoning => LlmModelFeature::Reasoning,
        ModelFeature::Tool => LlmModelFeature::Tool,
    }
}

fn to_llm_model_extra_info(
    config: &LlmConfig,
    extra_info: &ModelExtraInfo,
) -> LlmClientModelExtraInfo {
    let features = config
        .convert_features(&extra_info.features)
        .into_iter()
        .map(map_model_feature)
        .collect();

    LlmClientModelExtraInfo {
        name: extra_info.name.clone(),
        context_length: extra_info.context_length,
        output_max_tokens: extra_info.output_max_tokens,
        input_cost_per_1k: extra_info.input_cost_per_1k,
        output_cost_per_1k: extra_info.output_cost_per_1k,
        features,
        description: extra_info.description.clone(),
        input_modalities: extra_info
            .input_modalities
            .as_ref()
            .map(|modalities| config.convert_modalities(modalities)),
        output_modalities: extra_info
            .output_modalities
            .as_ref()
            .map(|modalities| config.convert_modalities(modalities)),
        metadata: extra_info.metadata.clone(),
        pricing: extra_info.pricing.clone(),
    }
}

impl LlmConfigProvider for LlmConfig {
    fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig> {
        self.get_provider_config(provider_type).and_then(|config| {
            let chat_api_type = LlmApiType::try_from(config.chat_api_type.as_str()).ok()?;
            let model_api_type = LlmModelApiType::try_from(config.model_api_type.as_str()).ok()?;

            let model_local = config.model_local.as_ref().map(|map| {
                map.iter()
                    .map(|(model_id, info)| (model_id.clone(), to_llm_model_extra_info(self, info)))
                    .collect()
            });

            Some(LlmProviderConfig {
                provider_type: config.provider_type.clone(),
                chat_api_type,
                model_api_type,
                model_local,
            })
        })
    }

    fn get_model_extra_info(
        &self,
        provider_type: &str,
        model_id: &str,
    ) -> Option<LlmClientModelExtraInfo> {
        self.get_model_extra_info(provider_type, model_id)
            .map(|info| to_llm_model_extra_info(self, info))
    }
}

/// 全局配置实例
static GLOBAL_LLM_CONFIG: OnceLock<LlmConfig> = OnceLock::new();

/// 获取全局 LLM 配置实例
pub fn get_global_llm_config() -> &'static LlmConfig {
    GLOBAL_LLM_CONFIG.get_or_init(|| LlmConfig::load())
}
