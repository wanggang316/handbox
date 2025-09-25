// MCP (Model Context Protocol) data models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

use crate::models::AppError;

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

/// Tool metadata returned from MCP servers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub annotations: HashMap<String, serde_json::Value>,
}

/// MCP server definition stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub status: McpServerStatus,
    #[serde(default)]
    pub tools: Vec<McpTool>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Request payload for creating a new MCP server
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
}

/// Request payload for updating an MCP server
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: Option<bool>,
}

/// Request payload for toggling an MCP server
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleMcpServerRequest {
    pub server_id: String,
    pub enabled: bool,
}

/// Request payload for refreshing MCP server metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshMcpServerRequest {
    pub server_id: String,
}
