// LLM 配置管理器
// 从 llm_config.json 读取供应商配置信息

use handbox_llm::config::{LlmConfigProvider, LlmProviderConfig};
use handbox_llm::types::{LlmApiType, LlmModelApiType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

/// Thinking Budget 选项配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<i32>, // -1 表示动态调整
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<i32>, // 0 表示禁用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<i32>>, // [min, max] 滑杆范围
}

/// Thinking Budget 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub models: Vec<String>,    // 适用的模型列表，格式: "provider_type/model_id"
    pub options: BudgetOptions, // 可选项
    pub default: String,        // 默认选项: "dynamic", "disable", "range"
}

/// 参数配置（合并了 default、max 和 UI 配置）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterConfig {
    pub component: Option<String>, // "slider" | "switch" | "responses_reasoning" | "completions_reasoning" | "thinking" | "openrouter_reasoning"
    pub level: Option<String>,     // "base" | "advance"
    pub step: Option<f64>,         // 仅滑块使用
    pub name: Option<String>,      // 显示名称
    pub show_toggle: Option<bool>, // 仅滑块使用，是否显示开关
    pub default: Option<Value>,    // 默认值
    pub max: Option<Value>,        // 最大值
    pub effort_options: Option<HashMap<String, Vec<String>>>, // reasoning 参数的 effort 选项
    pub summary_options: Option<HashMap<String, Vec<String>>>, // responses_reasoning 参数的 summary 选项
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>, // completions_reasoning 参数：是否包含推理过程
    pub budget_configs: Option<Vec<BudgetConfig>>,             // thinking 参数的 budget 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>, // 参数说明提示文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts_tip: Option<String>, // thinking 参数：包含过程的提示文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tip: Option<String>, // thinking 参数：预算模式的提示文本
    // OpenRouter reasoning 特定字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_tips: Option<String>, // openrouter_reasoning: effect 参数提示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_tips: Option<String>, // openrouter_reasoning: max_tokens 参数提示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_props: Option<Vec<String>>, // openrouter_reasoning: 默认展示的属性
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_props: Option<HashMap<String, Vec<String>>>, // openrouter_reasoning: 特殊模型的属性映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<Vec<i32>>, // openrouter_reasoning: max_tokens 范围 [min, max]
}

/// 聊天方法配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMethodConfig {
    #[serde(default)]
    pub default_supported_parameters: Vec<String>,
    #[serde(default)]
    pub additional_parameters: Vec<String>,
    #[serde(default)]
    pub parameters: HashMap<String, ParameterConfig>,
}

/// 聊天方法集合配置（包含 base 和各个方法）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMethodsConfig {
    #[serde(default)]
    pub base: ChatMethodConfig,
    #[serde(default)]
    pub completions: ChatMethodConfig,
    #[serde(default)]
    pub responses: ChatMethodConfig,
    #[serde(default)]
    pub google_generate_content: ChatMethodConfig,
}

impl ChatMethodsConfig {
    /// 获取合并后的方法配置
    pub fn get_merged_config(&self, method_name: &str) -> ChatMethodConfig {
        let method_config = match method_name {
            "completions" => &self.completions,
            "responses" => &self.responses,
            "google_generate_content" => &self.google_generate_content,
            _ => return self.base.clone(),
        };

        // 合并 base 和方法特定的配置
        let mut merged = self.base.clone();

        // 合并 default_supported_parameters（方法特定的覆盖 base）
        if !method_config.default_supported_parameters.is_empty() {
            merged.default_supported_parameters =
                method_config.default_supported_parameters.clone();
        }

        // 合并 additional_parameters（方法特定的覆盖 base）
        if !method_config.additional_parameters.is_empty() {
            merged.additional_parameters = method_config.additional_parameters.clone();
        }

        // 合并 parameters（方法特定的覆盖 base 中的同名参数）
        for (key, value) in &method_config.parameters {
            merged.parameters.insert(key.clone(), value.clone());
        }

        merged
    }
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
    pub model_api_type: String, // "openai" | "google" | "anthropic" | "openrouter"
    #[serde(default)]
    pub parameters: HashMap<String, ParameterConfig>, // 供应商级别的参数配置
}

/// LLM 配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub chat_methods: ChatMethodsConfig,
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
}

impl LlmConfig {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        Self {
            chat_methods: ChatMethodsConfig::default(),
            providers: Vec::new(),
            custom_providers: Vec::new(),
        }
    }

    /// 从配置文件加载（开发/测试用，按 cwd 相对路径读取）
    pub fn load() -> Self {
        let mut config = Self::new();
        match config.load_file_at(std::path::Path::new("llm_config.json")) {
            Ok(()) => {
                tracing::info!("Successfully loaded LLM config from llm_config.json");
            }
            Err(e) => {
                tracing::warn!("Failed to load config file: {}. Using empty config", e);
            }
        }
        config
    }

    /// 通过 Tauri AppHandle 从打包资源目录加载配置
    pub fn load_from_app(app: &tauri::AppHandle) -> Self {
        use tauri::Manager;
        let mut config = Self::new();
        let resource_dir = match app.path().resource_dir() {
            Ok(dir) => dir,
            Err(e) => {
                tracing::warn!("Failed to get resource dir: {}. Using empty config", e);
                return config;
            }
        };
        let path = resource_dir.join("llm_config.json");
        match config.load_file_at(&path) {
            Ok(()) => {
                tracing::info!("Successfully loaded LLM config from {}", path.display());
            }
            Err(e) => {
                tracing::warn!("Failed to load {}: {}. Using empty config", path.display(), e);
            }
        }
        config
    }

    /// 加载配置文件（按指定路径）
    fn load_file_at(&mut self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let config_content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {}: {}", path.display(), e))?;

        let loaded_config: LlmConfig = serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;

        self.chat_methods = loaded_config.chat_methods;
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

    /// 根据聊天方法名称获取配置（返回合并后的配置）
    pub fn get_chat_method_config(&self, method_name: &str) -> ChatMethodConfig {
        self.chat_methods.get_merged_config(method_name)
    }

    /// 获取合并后的配置（包含 provider 级别的参数覆盖）
    /// 合并顺序：base -> method -> provider（后者覆盖前者）
    pub fn get_merged_config_with_provider(
        &self,
        method_name: &str,
        provider_type: &str,
    ) -> ChatMethodConfig {
        // 首先获取 base + method 合并后的配置
        let mut merged = self.get_chat_method_config(method_name);

        // 查找 provider 配置
        let provider_config = self.get_provider_config(provider_type);

        // 如果 provider 有参数配置，覆盖到 merged 中
        if let Some(provider) = provider_config {
            for (key, value) in &provider.parameters {
                merged.parameters.insert(key.clone(), value.clone());
            }
        }

        merged
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

/// 安装全局 LLM 配置实例（应在使用前由启动流程调用）
pub fn install_global_llm_config(config: LlmConfig) {
    if GLOBAL_LLM_CONFIG.set(config).is_err() {
        tracing::warn!("Global LLM config already initialized; install_global_llm_config ignored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_parameter_config() {
        let config = LlmConfig::load();

        // 测试 responses 方法的 reasoning 参数
        let responses_config = config.get_chat_method_config("responses");
        let reasoning_param = responses_config.parameters.get("reasoning");
        assert!(reasoning_param.is_some());

        let reasoning = reasoning_param.unwrap();
        assert_eq!(reasoning.component, Some("responses_reasoning".to_string()));
        assert_eq!(reasoning.level, Some("base".to_string()));
        assert_eq!(reasoning.name, Some("Reasoning".to_string()));

        // 验证 effort_options
        assert!(reasoning.effort_options.is_some());
        let effort_options = reasoning.effort_options.as_ref().unwrap();
        assert!(effort_options.contains_key("common"));
        let common_effort = effort_options.get("common").unwrap();
        assert!(common_effort.contains(&"minimal".to_string()));
        assert!(common_effort.contains(&"low".to_string()));
        assert!(common_effort.contains(&"medium".to_string()));
        assert!(common_effort.contains(&"high".to_string()));

        // 验证 summary_options
        assert!(reasoning.summary_options.is_some());
        let summary_options = reasoning.summary_options.as_ref().unwrap();
        assert!(summary_options.contains_key("common"));
        let common_summary = summary_options.get("common").unwrap();
        assert!(common_summary.contains(&"auto".to_string()));
        assert!(common_summary.contains(&"detailed".to_string()));
    }

    #[test]
    fn test_completions_reasoning_parameter_config() {
        let config = LlmConfig::load();

        // 测试 completions 方法的 reasoning 参数
        let completions_config = config.get_chat_method_config("completions");
        let reasoning_param = completions_config.parameters.get("reasoning");
        assert!(reasoning_param.is_some());

        let reasoning = reasoning_param.unwrap();
        assert_eq!(
            reasoning.component,
            Some("completions_reasoning".to_string())
        );
        assert_eq!(reasoning.level, Some("base".to_string()));
        assert_eq!(reasoning.name, Some("Reasoning".to_string()));

        // 验证 effort_options
        assert!(reasoning.effort_options.is_some());
        let effort_options = reasoning.effort_options.as_ref().unwrap();
        assert!(effort_options.contains_key("common"));
        let common_effort = effort_options.get("common").unwrap();
        assert!(common_effort.contains(&"minimal".to_string()));
        assert!(common_effort.contains(&"low".to_string()));
        assert!(common_effort.contains(&"medium".to_string()));
        assert!(common_effort.contains(&"high".to_string()));

        // completions reasoning 不应该有 summary_options
        assert!(reasoning.summary_options.is_none());

        // 验证 include_reasoning
        assert_eq!(reasoning.include_reasoning, Some(true));
    }
}
