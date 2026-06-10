use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// Agent Session 实体 - Agent 模式下的会话实例
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: UUID,
    pub name: String,
    /// 所属 Agent Project（可选）。仅在创建时写入，之后不可经 update 改写。
    pub project_id: Option<UUID>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub working_dir: Option<String>,
    pub enabled_tools: Vec<String>, // JSON: Vec<String> (tool names)
    pub tool_execution_mode: Option<String>,
    pub message_count: i32,
    pub last_message_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Agent Session 消息 - payload 存储序列化后的 hand-agent Message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionMessage {
    pub id: UUID,
    pub session_id: UUID,
    pub seq: i64,
    pub role: String,
    pub payload: serde_json::Value,
    pub created_at: Timestamp,
}

/// 创建 Agent Session 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub name: String,
    /// 可选：挂靠到某个 Agent Project。提供时 working_dir 取 project.path
    /// （覆盖请求中的 working_dir），项目不存在 / 目录已失效则拒绝创建。
    pub project_id: Option<UUID>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub working_dir: Option<String>,
    pub enabled_tools: Option<Vec<String>>,
    pub tool_execution_mode: Option<String>,
}

/// 更新 Agent Session 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentSessionRequest {
    pub name: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub working_dir: Option<String>,
    pub enabled_tools: Option<Vec<String>>,
    pub tool_execution_mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_session_serialization_roundtrip() {
        let session = AgentSession {
            id: "agent_session_1".to_string(),
            name: "Coding Session".to_string(),
            project_id: Some("project_1".to_string()),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a coding agent.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            working_dir: Some("/tmp/project".to_string()),
            enabled_tools: vec!["read".to_string(), "write".to_string()],
            tool_execution_mode: Some("auto".to_string()),
            message_count: 3,
            last_message_at: Some(2000),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&session).expect("serialize");
        // Verify camelCase field naming on the wire.
        assert!(json.contains("\"modelId\""));
        assert!(json.contains("\"projectId\""));
        assert!(json.contains("\"enabledTools\""));
        assert!(json.contains("\"messageCount\""));
        assert!(json.contains("\"lastMessageAt\""));

        let deserialized: AgentSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.id, deserialized.id);
        assert_eq!(session.name, deserialized.name);
        assert_eq!(session.project_id, deserialized.project_id);
        assert_eq!(session.enabled_tools, deserialized.enabled_tools);
        assert_eq!(session.message_count, deserialized.message_count);
    }

    #[test]
    fn agent_session_message_serialization_roundtrip() {
        let message = AgentSessionMessage {
            id: "msg_1".to_string(),
            session_id: "agent_session_1".to_string(),
            seq: 1,
            role: "user".to_string(),
            payload: serde_json::json!({ "type": "text", "content": "hello" }),
            created_at: 1000,
        };

        let json = serde_json::to_string(&message).expect("serialize");
        // Verify camelCase field naming on the wire.
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"createdAt\""));

        let deserialized: AgentSessionMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.session_id, deserialized.session_id);
        assert_eq!(message.seq, deserialized.seq);
        assert_eq!(message.payload, deserialized.payload);
    }

    #[test]
    fn create_agent_session_request_partial() {
        let json = r#"{"name": "Test Session"}"#;
        let req: CreateAgentSessionRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "Test Session");
        assert!(req.project_id.is_none());
        assert!(req.model_id.is_none());
        assert!(req.enabled_tools.is_none());
    }
}
