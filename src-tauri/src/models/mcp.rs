use serde::Deserialize;
use std::collections::HashMap;

use crate::storage::types::McpConnectionType;

/// Request payload for creating a new MCP server
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMcpServerRequest {
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
    pub connection_type: Option<McpConnectionType>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub endpoint: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub timeout_ms: Option<u64>,
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

/// Request payload for updating tool enabled status
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateToolEnabledRequest {
    pub server_id: String,
    pub tool_name: String,
    pub enabled: bool,
}
