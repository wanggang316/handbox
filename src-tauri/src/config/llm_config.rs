// LLM 配置管理器
// 从 llm_config.json 读取供应商配置信息

use handbox_llm::config::{LlmConfigProvider, LlmProviderConfig};
use handbox_llm::types::{LlmApiType, LlmModelApiType, SupplementField};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

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
    pub model_api_type: String, // "openai" | "google" | "anthropic" | "openrouter"
    pub support_parameters: Option<Vec<String>>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
    pub supplement_file: Option<String>,
    pub supplement_fields: Option<Vec<SupplementField>>,
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
}

impl LlmConfigProvider for LlmConfig {
    fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig> {
        self.get_provider_config(provider_type).and_then(|config| {
            let chat_api_type = LlmApiType::try_from(config.chat_api_type.as_str()).ok()?;
            let model_api_type = LlmModelApiType::try_from(config.model_api_type.as_str()).ok()?;

            Some(LlmProviderConfig {
                provider_type: config.provider_type.clone(),
                chat_api_type,
                model_api_type,
                supplement_file: config.supplement_file.clone(),
                supplement_fields: config.supplement_fields.clone(),
            })
        })
    }
}

/// 全局配置实例
static GLOBAL_LLM_CONFIG: OnceLock<LlmConfig> = OnceLock::new();

/// 获取全局 LLM 配置实例
pub fn get_global_llm_config() -> &'static LlmConfig {
    GLOBAL_LLM_CONFIG.get_or_init(|| LlmConfig::load())
}
