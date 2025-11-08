use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

fn default_execution_mode() -> String {
    "auto".to_string()
}

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub server_id: String,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    #[serde(default)]
    pub enabled_tools: Vec<String>,
}

/// 聊天实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: UUID,
    pub name: String,
    pub last_message_at: Option<Timestamp>,
    pub message_count: i32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
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
    fn mcp_server_config_defaults() {
        let json = r#"{"serverId": "test"}"#;
        let config: McpServerConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.execution_mode, "auto");
        assert!(config.enabled_tools.is_empty());
    }

    #[test]
    fn chat_serialization_roundtrip() {
        let chat = Chat {
            id: "chat_1".to_string(),
            name: "Test".to_string(),
            last_message_at: Some(1000),
            message_count: 5,
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are helpful".to_string()),
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

        let json = serde_json::to_string(&chat).expect("serialize");
        let deserialized: Chat = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(chat.id, deserialized.id);
        assert_eq!(chat.name, deserialized.name);
    }
}
