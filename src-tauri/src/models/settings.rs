use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeColor {
    Blue,
    Green,
    Red,
    Yellow,
    Purple,
    Orange,
    Pink,
    Brown,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "zh-CN")]
    ZhCN,
    #[serde(rename = "en-US")]
    EnUS,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConfig {
    pub send_message: String,
    pub new_line: String,
    pub switch_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub theme: Theme,
    pub theme_color: ThemeColor,
    pub language: Language,
    pub auto_scroll: bool,
    /// macOS frosted-glass sidebar (behind-window vibrancy). Ignored on
    /// platforms without the effect; default on so configs written before the
    /// field existed pick up the native look.
    #[serde(default = "default_sidebar_vibrancy")]
    pub sidebar_vibrancy: bool,
    /// Navigation rail beside the transcript, one tick per question. Default
    /// on so configs written before the field existed still get the rail.
    #[serde(default = "default_message_nav")]
    pub message_nav: bool,
    pub shortcuts: ShortcutConfig,
}

fn default_sidebar_vibrancy() -> bool {
    true
}

fn default_message_nav() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettings {
    /// Cached chat session used for translation.
    pub session_id: Option<String>,
    /// Agent definition ID `session_id` was created with. `None` = built-in
    /// fallback (builtin-chat + hard-coded translation prompt). If it differs
    /// from `quickTools.translationAgentId`, the frontend recreates the session.
    #[serde(default)]
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    pub working_dir: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPSettings {
    pub servers: Vec<MCPServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub is_premium: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSettings {
    pub user: Option<UserInfo>,
    pub is_logged_in: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAppInfo {
    pub bundle_id: String,
    pub name: String,
    /// Icon as a base64 data URL (e.g. "data:image/png;base64,...").
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SelectionBlacklist {
    #[serde(default)]
    pub pids: Vec<i32>,
    #[serde(default)]
    pub apps: Vec<DisabledAppInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuickToolsSettings {
    #[serde(default)]
    pub show_toolbar_on_selection: bool,
    /// Agent definition ID for the selection "translate" tool. `None` =
    /// built-in fallback (builtin-chat + hard-coded translation prompt).
    #[serde(default)]
    pub translation_agent_id: Option<String>,
    #[serde(default)]
    pub selection_blacklist: SelectionBlacklist,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillSettings {
    /// Globally disabled skill names (exact match). Stored opaquely: orphaned,
    /// duplicate, or blank entries are kept verbatim — no normalization.
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Search-provider config for the agent `web_search` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchSettings {
    /// Search provider id. Only `"tavily"` is supported today; kept as a
    /// field for future providers.
    #[serde(default = "default_web_search_provider")]
    pub provider: String,
    /// Provider API key. Empty = unconfigured; the `web_search` tool is not
    /// registered.
    #[serde(default)]
    pub api_key: String,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            provider: default_web_search_provider(),
            api_key: String::new(),
        }
    }
}

fn default_web_search_provider() -> String {
    "tavily".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    /// Tools enabled by default for new agent sessions: the coding-agent
    /// built-ins plus HandBox's extension tools (`web_search` / `render_card` /
    /// `render_app` / `ask_question` / `skill`). All are on by default.
    #[serde(default = "default_agent_enabled_tools")]
    pub default_enabled_tools: Vec<String>,
    /// Default "Open in ..." target id (see commands/open_in.rs). `None` =
    /// unset; the frontend falls back to the first available editor/terminal.
    #[serde(default)]
    pub default_editor_id: Option<String>,
    /// Model new agent sessions start on. `None` = unset; the session is
    /// created without a model and the composer asks the user to pick one.
    /// Always written paired with [`Self::default_provider_id`].
    #[serde(default)]
    pub default_model_id: Option<String>,
    /// Provider owning [`Self::default_model_id`]; a model id alone is
    /// ambiguous because the same id can exist under several providers.
    #[serde(default)]
    pub default_provider_id: Option<String>,
    #[serde(default)]
    pub web_search: WebSearchSettings,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            default_enabled_tools: default_agent_enabled_tools(),
            default_editor_id: None,
            default_model_id: None,
            default_provider_id: None,
            web_search: WebSearchSettings::default(),
        }
    }
}

fn default_agent_enabled_tools() -> Vec<String> {
    [
        "read",
        "write",
        "edit",
        "bash",
        "grep",
        "find",
        "ls",
        "web_search",
        "render_card",
        "render_app",
        "ask_question",
        "skill",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// When a session's title is regenerated automatically. The manual
/// "generate title" action is unaffected by this rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TitleGenerationRule {
    /// Once, right after the session's first message.
    #[default]
    FirstMessage,
    /// After every message, re-titled from the conversation so far.
    EveryMessage,
    /// Never automatically.
    Off,
}

/// Session-level behaviour shared by every agent session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionSettings {
    #[serde(default)]
    pub title_generation: TitleGenerationRule,
}

/// How the quick-action overlay is summoned. The model it runs on is the
/// app-wide default in [`AgentSettings`], not a quick-action-specific one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickActionSettings {
    /// When disabled, the global shortcut is not registered.
    #[serde(default = "default_quick_action_enabled")]
    pub enabled: bool,
    /// Global shortcut that opens the quick-action panel (Tauri
    /// global-shortcut accelerator syntax).
    #[serde(default = "default_quick_action_shortcut")]
    pub shortcut: String,
}

impl Default for QuickActionSettings {
    fn default() -> Self {
        Self {
            enabled: default_quick_action_enabled(),
            shortcut: default_quick_action_shortcut(),
        }
    }
}

fn default_quick_action_enabled() -> bool {
    true
}

fn default_quick_action_shortcut() -> String {
    "CmdOrCtrl+Shift+Space".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub mcp: MCPSettings,
    pub account: AccountSettings,
    pub translation: TranslationSettings,
    #[serde(default)]
    pub quick_tools: QuickToolsSettings,
    #[serde(default)]
    pub skills: SkillSettings,
    #[serde(default)]
    pub agent: AgentSettings,
    #[serde(default)]
    pub quick_action: QuickActionSettings,
    #[serde(default)]
    pub session: SessionSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSettingsRequest {
    pub section: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportSettingsOptions {
    pub include_providers: Option<bool>,
    pub include_mcp: Option<bool>,
    pub include_shortcuts: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportSettingsRequest {
    pub data: String,
    pub overwrite: Option<bool>,
    pub sections: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // The hard-coded default accelerator stays in sync between the field
    // default fn and the manual Default impl.
    #[test]
    fn quick_action_default_shortcut() {
        assert_eq!(
            QuickActionSettings::default().shortcut,
            "CmdOrCtrl+Shift+Space"
        );
    }

    // A `quickAction` section present but missing the `shortcut` field falls
    // back to the default accelerator via serde(default) on the field.
    #[test]
    fn quick_action_missing_field_uses_default() {
        let parsed: QuickActionSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(parsed.shortcut, "CmdOrCtrl+Shift+Space");
    }

    // The field serializes/deserializes under its camelCase JSON key.
    #[test]
    fn quick_action_uses_camel_case_key() {
        let value = serde_json::to_value(QuickActionSettings::default()).unwrap();
        assert_eq!(value["shortcut"], "CmdOrCtrl+Shift+Space");

        let parsed: QuickActionSettings =
            serde_json::from_value(serde_json::json!({ "shortcut": "Alt+Space" })).unwrap();
        assert_eq!(parsed.shortcut, "Alt+Space");
    }

    // A `quickAction` section written while the overlay still had its own
    // default-model pair keeps parsing: the retired keys are ignored, not
    // rejected, so an old config does not reset the section to defaults.
    #[test]
    fn quick_action_retired_model_keys_are_ignored() {
        let parsed: QuickActionSettings = serde_json::from_value(serde_json::json!({
            "shortcut": "Alt+Space",
            "modelId": "gpt-4o",
            "providerId": "openai",
        }))
        .unwrap();
        assert_eq!(parsed.shortcut, "Alt+Space");

        let value = serde_json::to_value(&parsed).unwrap();
        assert!(value.get("modelId").is_none());
        assert!(value.get("providerId").is_none());
    }

    // The agent default-model pair is unset by default, and an `agent` section
    // written before the fields existed upgrades to None instead of failing to
    // parse (which would reset the whole section to defaults).
    #[test]
    fn agent_default_model_and_provider_are_none() {
        let defaults = AgentSettings::default();
        assert_eq!(defaults.default_model_id, None);
        assert_eq!(defaults.default_provider_id, None);

        let parsed: AgentSettings =
            serde_json::from_value(serde_json::json!({ "defaultEnabledTools": ["read"] })).unwrap();
        assert_eq!(parsed.default_model_id, None);
        assert_eq!(parsed.default_provider_id, None);
        assert_eq!(parsed.default_enabled_tools, vec!["read".to_string()]);
    }

    // The pair round-trips under its camelCase JSON keys.
    #[test]
    fn agent_default_model_uses_camel_case_keys() {
        let parsed: AgentSettings = serde_json::from_value(serde_json::json!({
            "defaultModelId": "gpt-4o",
            "defaultProviderId": "openai-1",
        }))
        .unwrap();
        assert_eq!(parsed.default_model_id.as_deref(), Some("gpt-4o"));
        assert_eq!(parsed.default_provider_id.as_deref(), Some("openai-1"));

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["defaultModelId"], "gpt-4o");
        assert_eq!(value["defaultProviderId"], "openai-1");
    }

    // The enabled flag defaults to true, both via Default and when the field
    // is absent from a persisted section (old configs upgrade to "on").
    #[test]
    fn quick_action_enabled_defaults_to_true() {
        assert!(QuickActionSettings::default().enabled);

        let parsed: QuickActionSettings =
            serde_json::from_value(serde_json::json!({ "shortcut": "Alt+Space" })).unwrap();
        assert!(parsed.enabled);
    }

    // The enabled flag round-trips under its JSON key.
    #[test]
    fn quick_action_enabled_round_trips() {
        let parsed: QuickActionSettings =
            serde_json::from_value(serde_json::json!({ "enabled": false })).unwrap();
        assert!(!parsed.enabled);

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["enabled"], false);
    }

    // translationAgentId defaults to None when absent and round-trips under
    // its camelCase JSON key.
    #[test]
    fn quick_tools_translation_agent_id_defaults_and_round_trips() {
        let parsed: QuickToolsSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(parsed.translation_agent_id, None);

        let parsed: QuickToolsSettings =
            serde_json::from_value(serde_json::json!({ "translationAgentId": "agent-1" })).unwrap();
        assert_eq!(parsed.translation_agent_id.as_deref(), Some("agent-1"));

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["translationAgentId"], "agent-1");
    }

    // translation.agentId defaults to None when absent (old configs that only
    // carry sessionId upgrade cleanly) and round-trips under its camelCase key.
    #[test]
    fn translation_agent_id_defaults_and_round_trips() {
        let parsed: TranslationSettings =
            serde_json::from_value(serde_json::json!({ "sessionId": "s-1" })).unwrap();
        assert_eq!(parsed.agent_id, None);

        let parsed: TranslationSettings =
            serde_json::from_value(serde_json::json!({ "sessionId": "s-1", "agentId": "agent-1" }))
                .unwrap();
        assert_eq!(parsed.agent_id.as_deref(), Some("agent-1"));

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["agentId"], "agent-1");
    }

    // An `agent` section missing the `webSearch` field falls back to the
    // default provider with an empty key (old configs upgrade cleanly).
    #[test]
    fn agent_missing_web_search_field_uses_default() {
        let parsed: AgentSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(parsed.web_search.provider, "tavily");
        assert_eq!(parsed.web_search.api_key, "");
    }

    // webSearch round-trips under its camelCase JSON keys.
    #[test]
    fn agent_web_search_uses_camel_case_keys() {
        let parsed: AgentSettings = serde_json::from_value(serde_json::json!({
            "webSearch": { "provider": "tavily", "apiKey": "tvly-secret" }
        }))
        .unwrap();
        assert_eq!(parsed.web_search.api_key, "tvly-secret");

        let value = serde_json::to_value(&parsed).unwrap();
        assert_eq!(value["webSearch"]["provider"], "tavily");
        assert_eq!(value["webSearch"]["apiKey"], "tvly-secret");
    }

    // The default enabled-tool list carries the extension tools after the 7
    // built-ins.
    #[test]
    fn default_agent_tools_include_extension_tools() {
        assert_eq!(
            AgentSettings::default().default_enabled_tools,
            vec![
                "read",
                "write",
                "edit",
                "bash",
                "grep",
                "find",
                "ls",
                "web_search",
                "render_card",
                "render_app",
                "ask_question",
                "skill"
            ]
        );
    }

    // Title generation defaults to the historical behaviour (once, after the
    // first message), both via Default and when the field is absent from a
    // persisted section.
    #[test]
    fn session_title_generation_defaults_to_first_message() {
        assert_eq!(
            SessionSettings::default().title_generation,
            TitleGenerationRule::FirstMessage
        );

        let parsed: SessionSettings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(parsed.title_generation, TitleGenerationRule::FirstMessage);
    }

    // The rule round-trips under its camelCase JSON key and variant names.
    #[test]
    fn session_title_generation_round_trips() {
        for (json, rule) in [
            ("firstMessage", TitleGenerationRule::FirstMessage),
            ("everyMessage", TitleGenerationRule::EveryMessage),
            ("off", TitleGenerationRule::Off),
        ] {
            let parsed: SessionSettings =
                serde_json::from_value(serde_json::json!({ "titleGeneration": json })).unwrap();
            assert_eq!(parsed.title_generation, rule);

            let value = serde_json::to_value(&parsed).unwrap();
            assert_eq!(value["titleGeneration"], json);
        }
    }

    // An old config.json with no `session` section upgrades cleanly.
    #[test]
    fn app_settings_missing_session_section_uses_default() {
        let mut value = serde_json::to_value(AppSettings {
            general: GeneralSettings {
                theme: Theme::System,
                theme_color: ThemeColor::System,
                language: Language::ZhCN,
                auto_scroll: true,
                sidebar_vibrancy: true,
                message_nav: true,
                shortcuts: ShortcutConfig {
                    send_message: "Enter".to_string(),
                    new_line: "Shift+Enter".to_string(),
                    switch_model: None,
                },
            },
            mcp: MCPSettings {
                servers: Vec::new(),
            },
            account: AccountSettings {
                user: None,
                is_logged_in: false,
            },
            translation: TranslationSettings {
                session_id: None,
                agent_id: None,
            },
            quick_tools: QuickToolsSettings::default(),
            skills: SkillSettings::default(),
            agent: AgentSettings::default(),
            quick_action: QuickActionSettings::default(),
            session: SessionSettings::default(),
        })
        .unwrap();
        value.as_object_mut().unwrap().remove("session");

        let parsed: AppSettings = serde_json::from_value(value).unwrap();
        assert_eq!(
            parsed.session.title_generation,
            TitleGenerationRule::FirstMessage
        );
    }

    // A config.json written before the rail existed keeps it switched on, the
    // same way `sidebarVibrancy` back-fills the native look.
    #[test]
    fn general_settings_missing_message_nav_defaults_on() {
        let parsed: GeneralSettings = serde_json::from_value(serde_json::json!({
            "theme": "system",
            "themeColor": "system",
            "language": "zh-CN",
            "autoScroll": true,
            "shortcuts": {
                "sendMessage": "Enter",
                "newLine": "Shift+Enter",
                "switchModel": null,
            },
        }))
        .unwrap();
        assert!(parsed.message_nav);
        assert!(parsed.sidebar_vibrancy);
    }
}
