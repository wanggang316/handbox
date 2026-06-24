use super::common::{Timestamp, UUID};
use super::session::{SessionReasoningConfig, McpServerConfig};
use serde::{Deserialize, Serialize};

// Agent 推理配置 - 复用 Session 的推理配置
pub type AgentReasoningConfig = SessionReasoningConfig;

/// Agent 实体 - 可复用的 AI 助手配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: UUID,
    pub name: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub skills: Vec<String>,
    /// 是否启用生成式 UI。`None` 等同「关闭」（旧行 / NULL 列）。
    pub generative_ui: Option<bool>,
    /// 关联的 GenUI（具名 JSON-Render spec）id。`None` 表示未关联（旧行 / NULL 列）；
    /// 引用的 GenUI 被删除后由仓储层置空，悬挂 id 在前端表单中显示为「未关联」。
    pub genui_id: Option<UUID>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 Agent 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRequest {
    pub name: String,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub skills: Option<Vec<String>>,
    pub generative_ui: Option<bool>,
    pub genui_id: Option<UUID>,
}

/// 更新 Agent 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub skills: Option<Vec<String>>,
    pub generative_ui: Option<bool>,
    pub genui_id: Option<UUID>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_serialization_roundtrip() {
        let agent = Agent {
            id: "agent_1".to_string(),
            name: "Code Assistant".to_string(),
            model: Some("gpt-4".to_string()),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            reasoning: None,
            max_tokens: Some(2048),
            system_prompt: Some("You are a helpful coding assistant.".to_string()),
            mcp_servers: vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }],
            skills: vec!["code-analysis".to_string(), "refactoring".to_string()],
            generative_ui: Some(true),
            genui_id: None,
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&agent).expect("serialize");
        let deserialized: Agent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(agent.id, deserialized.id);
        assert_eq!(agent.name, deserialized.name);
        assert_eq!(agent.skills, deserialized.skills);
        assert_eq!(agent.generative_ui, deserialized.generative_ui);
    }

    /// 锁定 JS<->Rust 线缆键：serde camelCase 把 `generative_ui` 转成 `generativeUi`
    /// （小写 `i`），而非 `generativeUI`。前端 `generativeUi?: boolean` 必须与之匹配。
    /// 键名不符会通过纯 Rust 的 round-trip，却在边界静默丢值。
    #[test]
    fn agent_generative_ui_wire_key_is_camel_case() {
        let agent = Agent {
            id: "agent_1".to_string(),
            name: "Code Assistant".to_string(),
            model: None,
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            max_tokens: None,
            system_prompt: None,
            mcp_servers: vec![],
            skills: vec![],
            generative_ui: Some(true),
            genui_id: None,
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&agent).expect("serialize");
        assert!(
            json.contains("\"generativeUi\""),
            "expected wire key `generativeUi`, got: {json}"
        );
        assert!(
            !json.contains("\"generativeUI\""),
            "wire key must be `generativeUi` (lowercase i), not `generativeUI`: {json}"
        );
    }

    #[test]
    fn create_agent_request_partial() {
        let json = r#"{"name": "Test Agent"}"#;
        let req: CreateAgentRequest =
            serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "Test Agent");
        assert!(req.model.is_none());
        assert!(req.skills.is_none());
    }
}
