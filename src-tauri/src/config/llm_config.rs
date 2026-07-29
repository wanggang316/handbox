// Provider and chat-parameter configuration loaded from llm_config.json.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<i32>, // -1 = dynamic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<i32>, // 0 = disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<i32>>, // [min, max] slider range
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub models: Vec<String>, // model ids as "provider_type/model_id"
    pub options: BudgetOptions,
    pub default: String, // "dynamic" | "disable" | "range"
}

/// Parameter config merging default/max values with UI presentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterConfig {
    pub component: Option<String>, // "slider" | "switch" | "responses_reasoning" | "completions_reasoning" | "thinking" | "openrouter_reasoning"
    pub level: Option<String>,     // "base" | "advance"
    pub step: Option<f64>,         // slider only
    pub name: Option<String>,      // display name
    pub show_toggle: Option<bool>, // slider only: show the on/off toggle
    pub default: Option<Value>,
    pub max: Option<Value>,
    pub effort_options: Option<HashMap<String, Vec<String>>>, // reasoning: effort options
    pub summary_options: Option<HashMap<String, Vec<String>>>, // responses_reasoning: summary options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>, // completions_reasoning: include the reasoning trace
    pub budget_configs: Option<Vec<BudgetConfig>>,             // thinking: budget configs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>, // help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts_tip: Option<String>, // thinking: include-thoughts help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tip: Option<String>, // thinking: budget-mode help text
    // openrouter_reasoning-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_tips: Option<String>, // effect help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_tips: Option<String>, // max_tokens help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_props: Option<Vec<String>>, // props shown by default
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special_props: Option<HashMap<String, Vec<String>>>, // per-model prop overrides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<Vec<i32>>, // max_tokens [min, max] range
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMethodConfig {
    #[serde(default)]
    pub default_supported_parameters: Vec<String>,
    #[serde(default)]
    pub additional_parameters: Vec<String>,
    #[serde(default)]
    pub parameters: HashMap<String, ParameterConfig>,
}

/// Chat-method configs: shared `base` plus per-method overrides.
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
    /// Merge `base` with the named method's config (method wins).
    pub fn get_merged_config(&self, method_name: &str) -> ChatMethodConfig {
        let method_config = match method_name {
            "completions" => &self.completions,
            "responses" => &self.responses,
            "google_generate_content" => &self.google_generate_content,
            _ => return self.base.clone(),
        };

        let mut merged = self.base.clone();

        if !method_config.default_supported_parameters.is_empty() {
            merged.default_supported_parameters =
                method_config.default_supported_parameters.clone();
        }

        if !method_config.additional_parameters.is_empty() {
            merged.additional_parameters = method_config.additional_parameters.clone();
        }

        for (key, value) in &method_config.parameters {
            merged.parameters.insert(key.clone(), value.clone());
        }

        merged
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(rename = "type")]
    pub provider_type: String,
    pub type_name: String,
    pub default_name: String,
    // The provider's API endpoint is a fact owned by hand-ai's catalog, not
    // HandBox: `augment_with_hand_ai_providers` fills this from
    // `hand_ai_catalog::list_providers()` for every catalog provider, so the
    // hand-tuned entries in llm_config.json don't carry it. Custom providers
    // (openai-compatible / anthropic-compatible) are NOT in the catalog, so
    // theirs stays empty — the user supplies a base_url when adding the
    // provider.
    #[serde(default)]
    pub default_base_url: String,
    pub icon: String,
    #[serde(default)]
    pub parameters: HashMap<String, ParameterConfig>, // provider-level parameter overrides
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub chat_methods: ChatMethodsConfig,
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmConfig {
    pub fn new() -> Self {
        Self {
            chat_methods: ChatMethodsConfig::default(),
            providers: Vec::new(),
            custom_providers: Vec::new(),
        }
    }

    /// Load `llm_config.json` relative to the cwd (dev/test only).
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

    /// Load `llm_config.json` from the bundled resource directory.
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
    ///    HandBox doesn't hard-code it. Custom providers
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
                // Generic placeholder icon until per-provider art exists.
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

    pub fn get_provider_config(&self, provider_type: &str) -> Option<&ProviderConfig> {
        if let Some(config) = self
            .providers
            .iter()
            .find(|p| p.provider_type == provider_type)
        {
            return Some(config);
        }

        self.custom_providers
            .iter()
            .find(|p| p.provider_type == provider_type)
    }

    pub fn get_all_provider_configs(&self) -> Vec<&ProviderConfig> {
        let mut all_configs = Vec::new();
        all_configs.extend(self.providers.iter());
        all_configs.extend(self.custom_providers.iter());
        all_configs
    }

    /// Merged (base + method) config for a chat method.
    pub fn get_chat_method_config(&self, method_name: &str) -> ChatMethodConfig {
        self.chat_methods.get_merged_config(method_name)
    }

    /// Merge order: base -> method -> provider (later wins).
    pub fn get_merged_config_with_provider(
        &self,
        method_name: &str,
        provider_type: &str,
    ) -> ChatMethodConfig {
        let mut merged = self.get_chat_method_config(method_name);

        let provider_config = self.get_provider_config(provider_type);

        if let Some(provider) = provider_config {
            for (key, value) in &provider.parameters {
                merged.parameters.insert(key.clone(), value.clone());
            }
        }

        merged
    }
}

static GLOBAL_LLM_CONFIG: OnceLock<LlmConfig> = OnceLock::new();

/// Falls back to a cwd-relative load when `install_global_llm_config`
/// hasn't run (dev/test).
pub fn get_global_llm_config() -> &'static LlmConfig {
    GLOBAL_LLM_CONFIG.get_or_init(LlmConfig::load)
}

/// Install the global config; startup must call this before first use.
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

        let responses_config = config.get_chat_method_config("responses");
        let reasoning_param = responses_config.parameters.get("reasoning");
        assert!(reasoning_param.is_some());

        let reasoning = reasoning_param.unwrap();
        assert_eq!(reasoning.component, Some("responses_reasoning".to_string()));
        assert_eq!(reasoning.level, Some("base".to_string()));
        assert_eq!(reasoning.name, Some("Reasoning".to_string()));

        assert!(reasoning.effort_options.is_some());
        let effort_options = reasoning.effort_options.as_ref().unwrap();
        assert!(effort_options.contains_key("common"));
        let common_effort = effort_options.get("common").unwrap();
        assert!(common_effort.contains(&"minimal".to_string()));
        assert!(common_effort.contains(&"low".to_string()));
        assert!(common_effort.contains(&"medium".to_string()));
        assert!(common_effort.contains(&"high".to_string()));

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

        assert!(reasoning.effort_options.is_some());
        let effort_options = reasoning.effort_options.as_ref().unwrap();
        assert!(effort_options.contains_key("common"));
        let common_effort = effort_options.get("common").unwrap();
        assert!(common_effort.contains(&"minimal".to_string()));
        assert!(common_effort.contains(&"low".to_string()));
        assert!(common_effort.contains(&"medium".to_string()));
        assert!(common_effort.contains(&"high".to_string()));

        assert!(reasoning.summary_options.is_none());

        assert_eq!(reasoning.include_reasoning, Some(true));
    }
}
