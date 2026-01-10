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
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    /// 目标语言，支持 "system" 或 IETF 语言标签（如 "en-US"）
    pub target_language: String,
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

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub mcp: MCPSettings,
    pub account: AccountSettings,
    pub translation: TranslationSettings,
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
