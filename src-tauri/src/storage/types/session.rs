use super::common::{Timestamp, UUID};
use crate::models::llm_types::{LlmReasoningEffortConfig, LlmResponsesReasoning, LlmThinkingConfig};
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

/// Session 实体（Agent 的实例）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
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
    pub agent_id: Option<UUID>, // 关联的 Agent ID
    pub reasoning: Option<SessionReasoningConfig>,
    /// 生成式 UI 开关，会话创建时由 Agent 快照而来（write-once）。
    /// `None` 等同「关闭」（旧行 / NULL 列）。
    pub generative_ui: Option<bool>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// OpenRouter 推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmOpenrouterReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,
}

/// Session 级推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<LlmResponsesReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<LlmReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<LlmThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<LlmOpenrouterReasoning>,
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
    fn session_serialization_roundtrip() {
        let session = Session {
            id: "session_1".to_string(),
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
            agent_id: None,
            reasoning: None,
            generative_ui: Some(true),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&session).expect("serialize");
        let deserialized: Session = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.id, deserialized.id);
        assert_eq!(session.name, deserialized.name);
        assert_eq!(session.generative_ui, deserialized.generative_ui);
    }

    /// 锁定 JS<->Rust 线缆键：serde camelCase 把 `generative_ui` 转成 `generativeUi`
    /// （小写 `i`），而非 `generativeUI`。前端 `generativeUi?: boolean` 必须与之匹配。
    /// 键名不符会通过纯 Rust 的 round-trip，却在边界静默丢值。
    #[test]
    fn session_generative_ui_wire_key_is_camel_case() {
        let session = Session {
            id: "session_1".to_string(),
            name: "Test".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            generative_ui: Some(true),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&session).expect("serialize");
        let deserialized: Session = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.generative_ui, deserialized.generative_ui);
        assert!(
            json.contains("\"generativeUi\""),
            "expected wire key `generativeUi`, got: {json}"
        );
        assert!(
            !json.contains("\"generativeUI\""),
            "wire key must be `generativeUi` (lowercase i), not `generativeUI`: {json}"
        );
    }
}
