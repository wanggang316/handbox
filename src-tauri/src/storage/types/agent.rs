use super::common::{Timestamp, UUID};
use super::chat::{ChatReasoningConfig, McpServerConfig};
use serde::{Deserialize, Serialize};

// Agent 推理配置 - 复用 Chat 的推理配置
pub type AgentReasoningConfig = ChatReasoningConfig;

/// Agent 实体 - 可复用的 AI 助手配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: UUID,
    pub name: String,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub streaming: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub skills: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 Agent 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRequest {
    pub name: String,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub streaming: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub skills: Option<Vec<String>>,
}

/// 更新 Agent 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub streaming: Option<bool>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub skills: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_serialization_roundtrip() {
        let agent = Agent {
            id: "agent_1".to_string(),
            name: "Code Assistant".to_string(),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            reasoning: None,
            max_tokens: Some(2048),
            streaming: Some(true),
            system_prompt: Some("You are a helpful coding assistant.".to_string()),
            mcp_servers: vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }],
            skills: vec!["code-analysis".to_string(), "refactoring".to_string()],
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&agent).expect("serialize");
        let deserialized: Agent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(agent.id, deserialized.id);
        assert_eq!(agent.name, deserialized.name);
        assert_eq!(agent.skills, deserialized.skills);
    }

    #[test]
    fn create_agent_request_partial() {
        let json = r#"{"name": "Test Agent"}"#;
        let req: CreateAgentRequest =
            serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "Test Agent");
        assert!(req.model_id.is_none());
        assert!(req.skills.is_none());
    }
}
