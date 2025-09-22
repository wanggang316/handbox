// LLM 配置管理器
// 从 llm_config.json 读取供应商和模型配置信息

use crate::models::provider::ModelFeature;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

/// 模型额外信息配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExtraInfo {
    pub name: String,
    pub context_length: Option<i32>,
    pub input_cost_per_1k: Option<f32>,
    pub output_cost_per_1k: Option<f32>,
    pub features: Vec<String>,
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
}

/// LLM 配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
}

impl LlmConfig {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            custom_providers: Vec::new(),
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

    /// 转换特性字符串为ModelFeature枚举
    pub fn convert_features(&self, features: &[String]) -> Vec<ModelFeature> {
        features
            .iter()
            .filter_map(|f| match f.as_str() {
                "text" => Some(ModelFeature::Text),
                "vision" => Some(ModelFeature::Vision),
                "function_calling" => Some(ModelFeature::FunctionCalling),
                "streaming" => Some(ModelFeature::Streaming),
                "reasoning" => Some(ModelFeature::Reasoning),
                _ => None,
            })
            .collect()
    }
}

/// 全局配置实例
static GLOBAL_LLM_CONFIG: OnceLock<LlmConfig> = OnceLock::new();

/// 获取全局 LLM 配置实例
pub fn get_global_llm_config() -> &'static LlmConfig {
    GLOBAL_LLM_CONFIG.get_or_init(|| LlmConfig::load())
}
