use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use crate::models::AppError;

/// MCP connection type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpConnectionType {
    Stdio,
    Sse,
    Http,
}

impl McpConnectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpConnectionType::Stdio => "stdio",
            McpConnectionType::Sse => "sse",
            McpConnectionType::Http => "http",
        }
    }
}

impl std::fmt::Display for McpConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for McpConnectionType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "stdio" => Ok(McpConnectionType::Stdio),
            "sse" => Ok(McpConnectionType::Sse),
            "http" => Ok(McpConnectionType::Http),
            other => Err(AppError::validation_error(&format!(
                "Unknown MCP connection type: {}",
                other
            ))),
        }
    }
}

impl From<&str> for McpConnectionType {
    fn from(value: &str) -> Self {
        McpConnectionType::from_str(value).unwrap_or(McpConnectionType::Stdio)
    }
}

impl Default for McpConnectionType {
    fn default() -> Self {
        McpConnectionType::Stdio
    }
}

/// MCP server status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatus {
    Inactive,
    Ready,
    Error,
    Unknown,
}

impl McpServerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            McpServerStatus::Inactive => "inactive",
            McpServerStatus::Ready => "ready",
            McpServerStatus::Error => "error",
            McpServerStatus::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for McpServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for McpServerStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "inactive" => Ok(McpServerStatus::Inactive),
            "ready" | "active" => Ok(McpServerStatus::Ready),
            "error" | "failed" => Ok(McpServerStatus::Error),
            "unknown" => Ok(McpServerStatus::Unknown),
            other => Err(AppError::validation_error(&format!(
                "Unknown MCP server status: {}",
                other
            ))),
        }
    }
}

impl From<&str> for McpServerStatus {
    fn from(value: &str) -> Self {
        McpServerStatus::from_str(value).unwrap_or(McpServerStatus::Unknown)
    }
}

pub use handbox_mcp::types::{McpErrorDetail, McpPrompt, McpPromptArgument, McpResource, McpTool};

/// MCP server definition stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub connection_type: McpConnectionType,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub endpoint: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub enabled: bool,
    pub status: McpServerStatus,
    #[serde(default)]
    pub tools: Vec<McpTool>,
    #[serde(default)]
    pub prompts: Vec<McpPrompt>,
    #[serde(default)]
    pub resources: Vec<McpResource>,
    #[serde(default)]
    pub enabled_tools: Vec<String>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<McpErrorDetail>,
    pub created_at: i64,
    pub updated_at: i64,
}
