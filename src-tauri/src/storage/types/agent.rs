use super::common::{Timestamp, UUID};
use super::mcp::McpServerConfig;
use crate::models::llm_types::SessionReasoningConfig;
use serde::{Deserialize, Serialize};

pub type AgentReasoningConfig = SessionReasoningConfig;

/// A reusable AI assistant configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: UUID,
    pub name: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub reasoning: Option<AgentReasoningConfig>,
    pub max_tokens: Option<i32>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub skills: Vec<String>,
    /// Whether generative UI is enabled. `None` (legacy rows / NULL column)
    /// means off.
    pub generative_ui: Option<bool>,
    /// Linked GenUI (named JSON-Render spec) id. `None` = unlinked (legacy
    /// rows / NULL column). The repository clears it when the referenced GenUI
    /// is deleted; a dangling id shows as "unlinked" in the frontend form.
    pub genui_id: Option<UUID>,
    /// Selected provider id. `None` (legacy/builtin rows) = picked in the UI
    /// at instantiation time.
    pub provider_id: Option<String>,
    /// Lucide icon name.
    pub icon: Option<String>,
    pub description: Option<String>,
    /// Built-in AgentDefinition (`builtin-chat` / `builtin-coding`). Built-in
    /// rows are protected in the service layer: no delete, no rename. NULL/0
    /// legacy rows decode as `false`.
    pub builtin: bool,
    /// Enabled built-in tool names (coding-agent registry names:
    /// read/write/edit/bash/grep/find/ls). Empty = chat-only, no built-in
    /// tools registered. NULL columns decode as an empty `Vec`.
    pub builtin_tools: Vec<String>,
    /// `"required"` | `"optional"` | `"none"`. `None` legacy rows behave as
    /// optional.
    pub working_dir_mode: Option<String>,
    /// Default tool execution policy: `"auto"` | `"manual"`. `None` legacy
    /// rows behave as auto.
    pub tool_execution_mode: Option<String>,
    /// coding-agent thinking level. `None` = engine default.
    pub thinking_level: Option<String>,
    /// Starter prompts. NULL columns decode as an empty `Vec`.
    pub starters: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// `Default` lets callers set only the fields they care about
/// (`CreateAgentRequest { name, ..Default::default() }`). `builtin` is absent
/// on purpose: user-created agents are never built-in; built-in rows are only
/// seeded by migrations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRequest {
    pub name: String,
    pub provider_id: Option<String>,
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
    pub icon: Option<String>,
    pub description: Option<String>,
    pub builtin_tools: Option<Vec<String>>,
    pub working_dir_mode: Option<String>,
    pub tool_execution_mode: Option<String>,
    pub thinking_level: Option<String>,
    pub starters: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
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
            provider_id: None,
            icon: None,
            description: None,
            builtin: false,
            builtin_tools: vec![],
            working_dir_mode: None,
            tool_execution_mode: None,
            thinking_level: None,
            starters: vec![],
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

    /// serde camelCase maps `generative_ui` to `generativeUi` (lowercase `i`),
    /// not `generativeUI`; the frontend's `generativeUi?: boolean` must match.
    /// A mismatched key survives a pure-Rust round-trip but silently drops the
    /// value at the JS boundary.
    #[test]
    fn agent_generative_ui_wire_key_is_camel_case() {
        let agent = Agent {
            id: "agent_1".to_string(),
            name: "Code Assistant".to_string(),
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
            provider_id: None,
            icon: None,
            description: None,
            builtin: false,
            builtin_tools: vec![],
            working_dir_mode: None,
            tool_execution_mode: None,
            thinking_level: None,
            starters: vec![],
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
        assert!(req.skills.is_none());
    }
}
