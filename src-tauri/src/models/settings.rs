// 设置相关数据模型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 主题类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

/// 主题色
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

/// 语言
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "zh-CN")]
    ZhCN,
    #[serde(rename = "en-US")]
    EnUS,
}

/// 快捷键配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConfig {
    pub send_message: String,
    pub new_line: String,
    pub switch_model: Option<String>,
}

/// 通用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub theme: Theme,
    pub theme_color: ThemeColor,
    pub language: Language,
    pub auto_scroll: bool,
    pub shortcuts: ShortcutConfig,
}

/// 翻译设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationSettings {
    /// 翻译使用的 Session ID
    pub session_id: Option<String>,
}

/// MCP 服务器配置
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

/// MCP 设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPSettings {
    pub servers: Vec<MCPServer>,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub is_premium: Option<bool>,
}

/// 账户设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSettings {
    pub user: Option<UserInfo>,
    pub is_logged_in: bool,
}

/// 禁用的应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledAppInfo {
    pub bundle_id: String,
    pub name: String,
    /// 图标数据的 base64 编码（data URL 格式，如 "data:image/png;base64,..."）
    pub icon: Option<String>,
}

/// 快捷工具设置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SelectionBlacklist {
    #[serde(default)]
    pub pids: Vec<i32>,
    #[serde(default)]
    pub apps: Vec<DisabledAppInfo>,
}

/// 快捷工具设置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuickToolsSettings {
    /// 选中文本时显示工具栏
    #[serde(default)]
    pub show_toolbar_on_selection: bool,
    /// 选词工具黑名单
    #[serde(default)]
    pub selection_blacklist: SelectionBlacklist,
}

/// Skill 设置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillSettings {
    /// 全局禁用的 skill 名单（按 name 精确匹配；不透明存储：孤儿/重复/空白
    /// 条目原样保留，不归一化、不去重、不清理）
    #[serde(default)]
    pub disabled: Vec<String>,
}

/// Agent 设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    /// 新建 Agent 会话默认启用的内置工具(coding-agent 注册名)。默认全 7 个。
    #[serde(default = "default_agent_enabled_tools")]
    pub default_enabled_tools: Vec<String>,
    /// "Open in ..." 的默认应用 target id（见 commands/open_in.rs）。
    /// `None` = 未设默认，前端回退到首个可用 editor/terminal。
    #[serde(default)]
    pub default_editor_id: Option<String>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            default_enabled_tools: default_agent_enabled_tools(),
            default_editor_id: None,
        }
    }
}

fn default_agent_enabled_tools() -> Vec<String> {
    ["read", "write", "edit", "bash", "grep", "find", "ls"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// 快捷动作设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickActionSettings {
    /// 唤起快捷动作面板的全局快捷键(Tauri global-shortcut 加速键语法)。
    #[serde(default = "default_quick_action_shortcut")]
    pub shortcut: String,
}

impl Default for QuickActionSettings {
    fn default() -> Self {
        Self {
            shortcut: default_quick_action_shortcut(),
        }
    }
}

fn default_quick_action_shortcut() -> String {
    "CmdOrCtrl+Shift+Space".to_string()
}

/// 应用设置
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
}

/// 设置更新请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSettingsRequest {
    pub section: String,
    pub data: serde_json::Value,
}

/// 导出设置选项
#[derive(Debug, Clone, Deserialize)]
pub struct ExportSettingsOptions {
    pub include_providers: Option<bool>,
    pub include_mcp: Option<bool>,
    pub include_shortcuts: Option<bool>,
}

/// 导入设置请求
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
}
