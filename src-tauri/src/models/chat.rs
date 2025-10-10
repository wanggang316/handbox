// 聊天相关数据模型

use serde::{Deserialize, Serialize};

pub type UUID = String;
pub type Timestamp = i64;

/// 模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub context_length: Option<i32>,
    pub stream: Option<bool>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            context_length: Some(4096),
            stream: Some(true),
        }
    }
}

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub server_id: String,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String, // "auto" or "manual"
    #[serde(default)]
    pub enabled_tools: Vec<String>, // List of enabled tool names for this server
}

fn default_execution_mode() -> String {
    "auto".to_string()
}

/// 聊天实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: UUID,
    pub name: String,
    pub last_message_at: Option<Timestamp>,
    pub message_count: i32,

    // Chat-level configuration (default values)
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub turn_count: Option<i32>,

    pub artifact_id: Option<UUID>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_serialization_roundtrip() {
        let chat = Chat {
            id: "chat_123".to_string(),
            name: "Test Chat".to_string(),
            last_message_at: Some(1000),
            message_count: 5,
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }],
            turn_count: Some(5),
            artifact_id: None,
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&chat).expect("serialize chat");
        let deserialized: Chat = serde_json::from_str(&json).expect("deserialize chat");

        assert_eq!(chat.id, deserialized.id);
        assert_eq!(chat.name, deserialized.name);
        assert_eq!(chat.message_count, deserialized.message_count);
    }

    #[test]
    fn mcp_server_config_default_execution_mode() {
        let json = r#"{"serverId": "test-server"}"#;
        let config: McpServerConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.execution_mode, "auto");
    }

    #[test]
    fn mcp_server_config_with_execution_mode() {
        let config = McpServerConfig {
            server_id: "test-server".to_string(),
            execution_mode: "manual".to_string(),
            enabled_tools: vec!["tool1".to_string(), "tool2".to_string()],
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: McpServerConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.server_id, "test-server");
        assert_eq!(deserialized.execution_mode, "manual");
        assert_eq!(
            deserialized.enabled_tools,
            vec!["tool1".to_string(), "tool2".to_string()]
        );
    }

    #[test]
    fn mcp_server_config_default_enabled_tools() {
        let json = r#"{"serverId": "test-server", "executionMode": "auto"}"#;
        let config: McpServerConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.enabled_tools, Vec::<String>::new());
    }

    #[test]
    fn model_parameters_default_values() {
        let params = ModelParameters::default();

        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.max_tokens, Some(2048));
        assert_eq!(params.context_length, Some(4096));
        assert_eq!(params.stream, Some(true));
    }
}
