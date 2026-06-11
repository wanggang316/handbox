// LLM 配置管理器
// 从 llm_config.json 读取供应商配置信息

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
    // The provider's API endpoint is a fact owned by hand-ai's catalog, not
    // HandBox: `augment_with_hand_ai_providers` fills this from
    // `hand_ai_catalog::list_providers()` for every catalog provider, so the
    // hand-tuned entries in llm_config.json no longer carry it. Custom
    // providers (openai-compatible / anthropic-compatible) are NOT in the
    // catalog, so theirs stays empty — the user supplies a base_url when
    // adding the provider.
    #[serde(default)]
    pub default_base_url: String,
    pub icon: String,
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
        config.augment_with_hand_ai_providers();
        config
    }

    /// Merge hand-ai's catalog into the loaded config:
    ///
    /// 1. **Fill endpoints.** For every provider already present (the
    ///    hand-tuned entries in llm_config.json) whose `default_base_url` is
    ///    empty, fill it from the catalog. The endpoint is hand-ai's fact, so
    ///    HandBox no longer hard-codes it. Custom providers
    ///    (openai-compatible / anthropic-compatible) aren't in the catalog and
    ///    keep their empty base_url — the user supplies one when adding them.
    /// 2. **Append catalog-only providers.** Synthesize a `ProviderConfig`
    ///    for every catalog provider not already present, so the
    ///    `get_provider_configs` IPC and `LlmConfig::get_provider_config`
    ///    lookups surface the 30+ vendors hand-ai knows about (Bedrock, Groq,
    ///    xAI, Cerebras, etc.) without HandBox maintaining its own catalog.
    fn augment_with_hand_ai_providers(&mut self) {
        let catalog = crate::services::hand_ai_catalog::list_providers();

        // Single source of truth for provider endpoints: provider_type -> base_url.
        let base_url_by_type: HashMap<String, String> = catalog
            .iter()
            .map(|hp| (hp.id.clone(), hp.default_base_url.clone()))
            .collect();

        // 1. Fill empty endpoints on existing (hand-tuned) entries from the catalog.
        for p in self
            .providers
            .iter_mut()
            .chain(self.custom_providers.iter_mut())
        {
            if p.default_base_url.is_empty() {
                if let Some(url) = base_url_by_type.get(&p.provider_type) {
                    p.default_base_url = url.clone();
                }
            }
        }

        // 2. Append catalog providers not already present.
        let existing: std::collections::HashSet<String> = self
            .providers
            .iter()
            .chain(self.custom_providers.iter())
            .map(|p| p.provider_type.clone())
            .collect();
        let mut appended = 0usize;
        for hp in catalog {
            if existing.contains(&hp.id) {
                continue;
            }
            let display_name = humanize_id(&hp.id);
            self.providers.push(ProviderConfig {
                provider_type: hp.id.clone(),
                type_name: display_name.clone(),
                default_name: display_name,
                default_base_url: hp.default_base_url.clone(),
                // Generic placeholder — `static/logo-150.png` exists; per-
                // provider art lands when a designer touches each one.
                // Tracked as a deferred decision in the overnight summary.
                icon: "/logo-150.png".to_string(),
                parameters: std::collections::HashMap::new(),
            });
            appended += 1;
        }
        if appended > 0 {
            tracing::info!(
                "Augmented LLM config with {} hand-ai providers ({} total now)",
                appended,
                self.providers.len(),
            );
        }
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

/// Format a kebab-case provider id (e.g. `"github-copilot"`) into a
/// space-separated, title-cased display name (`"Github Copilot"`). Used
/// when synthesizing `ProviderConfig` entries for hand-ai-only providers
/// that don't have hand-tuned metadata in `llm_config.json`.
fn humanize_id(id: &str) -> String {
    id.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_id_handles_kebab_and_single_word() {
        assert_eq!(humanize_id("openai"), "Openai");
        assert_eq!(humanize_id("github-copilot"), "Github Copilot");
        assert_eq!(humanize_id("amazon-bedrock"), "Amazon Bedrock");
        assert_eq!(humanize_id("xiaomi-token-plan-cn"), "Xiaomi Token Plan Cn");
        assert_eq!(humanize_id(""), "");
    }

    #[test]
    fn augment_appends_hand_ai_providers_without_clobbering_existing() {
        let mut cfg = LlmConfig::new();
        // Pretend llm_config.json had a single hand-tuned openai entry with a
        // custom icon and NO endpoint (the endpoint is hand-ai's fact, filled
        // by augmentation). Augmentation must NOT replace the hand-tuned
        // metadata with a synthesized version, but MUST fill the endpoint.
        cfg.providers.push(ProviderConfig {
            provider_type: "openai".into(),
            type_name: "OpenAI".into(),
            default_name: "OpenAI".into(),
            default_base_url: String::new(),
            icon: "/logo-openai.png".into(),
            parameters: std::collections::HashMap::new(),
        });
        let before = cfg.providers.len();
        cfg.augment_with_hand_ai_providers();
        let after = cfg.providers.len();
        assert!(
            after > before,
            "augmentation should add hand-ai-only providers"
        );

        let openai = cfg
            .providers
            .iter()
            .find(|p| p.provider_type == "openai")
            .unwrap();
        assert_eq!(
            openai.type_name, "OpenAI",
            "hand-tuned name must survive augmentation"
        );
        assert_eq!(
            openai.default_base_url, "https://api.openai.com/v1",
            "empty endpoint on a hand-tuned entry must be filled from the catalog"
        );

        // Spot-check that a hand-ai-only provider got synthesized in.
        assert!(
            cfg.providers.iter().any(|p| p.provider_type == "groq"),
            "groq (hand-ai-only) should now appear"
        );
        let groq = cfg
            .providers
            .iter()
            .find(|p| p.provider_type == "groq")
            .unwrap();
        assert_eq!(groq.type_name, "Groq");
        // Generic placeholder until per-provider art is added.
        assert_eq!(groq.icon, "/logo-150.png");
    }

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
