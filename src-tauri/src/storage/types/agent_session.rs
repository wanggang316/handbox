use super::common::{Timestamp, UUID};
use super::mcp::McpServerConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: UUID,
    pub name: String,
    /// Owning Agent Project. Written only at creation; never changed via update.
    pub project_id: Option<UUID>,
    /// AgentDefinition this session was instantiated from (set by
    /// `create_session_from_definition`; `None` for direct `create_session` or
    /// legacy sessions). Creation-only, like `project_id`: the generic update
    /// path never writes it.
    pub agent_definition_id: Option<UUID>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub working_dir: Option<String>,
    pub enabled_tools: Vec<String>, // JSON: Vec<String> (tool names)
    /// Per-session MCP server bindings (JSON). Empty = no MCP tools injected.
    pub mcp_servers: Vec<McpServerConfig>,
    pub tool_execution_mode: Option<String>,
    pub message_count: i32,
    pub last_message_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// `payload` stores a serialized hand-agent Message.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentSessionRequest {
    pub name: String,
    /// Optional Agent Project to attach to. When set, working_dir comes from
    /// project.path (overriding the request's working_dir); creation is
    /// rejected if the project is missing or its directory is gone.
    pub project_id: Option<UUID>,
    /// Source AgentDefinition id, filled by `create_session_from_definition`;
    /// frontend paths calling `create_session` directly leave it empty.
    pub agent_definition_id: Option<UUID>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub thinking_level: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub working_dir: Option<String>,
    pub enabled_tools: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub tool_execution_mode: Option<String>,
}

/// Instantiates a session from an AgentDefinition.
///
/// The definition supplies the capability set (builtin_tools / mcp_servers)
/// and defaults (model / provider / system_prompt / sampling / thinking_level /
/// tool_execution_mode); this request only carries overrides decided at
/// instantiation time. `Default` lets the frontend fill just what it needs.
///
/// - `name`: empty falls back to definition.name.
/// - `project_id` / `working_dir`: constrained by definition.working_dir_mode
///   (`required`: exactly one must be given; `none`: always ignored;
///   `optional`: used when present).
/// - `model_id` / `provider_id`: override the definition defaults (the builtin
///   chat definition has no provider, so one must be picked here).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstantiateAgentSessionRequest {
    pub name: Option<String>,
    pub project_id: Option<UUID>,
    pub working_dir: Option<String>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
}

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
    pub mcp_servers: Option<Vec<McpServerConfig>>,
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
            agent_definition_id: Some("builtin-coding".to_string()),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a coding agent.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            working_dir: Some("/tmp/project".to_string()),
            enabled_tools: vec!["read".to_string(), "write".to_string()],
            mcp_servers: Vec::new(),
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
        assert!(json.contains("\"agentDefinitionId\""));
        assert!(json.contains("\"enabledTools\""));
        // The deprecated enabledSkills key must be ABSENT from the wire JSON.
        assert!(!json.contains("\"enabledSkills\""));
        assert!(json.contains("\"messageCount\""));
        assert!(json.contains("\"lastMessageAt\""));

        let deserialized: AgentSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(session.id, deserialized.id);
        assert_eq!(session.name, deserialized.name);
        assert_eq!(session.project_id, deserialized.project_id);
        assert_eq!(session.agent_definition_id, deserialized.agent_definition_id);
        assert_eq!(session.enabled_tools, deserialized.enabled_tools);
        assert_eq!(session.message_count, deserialized.message_count);
    }

    /// A messageless session must serialize `lastMessageAt` as JSON null — never 0.
    /// A literal 0 short-circuits the frontend's `lastMessageAt ?? createdAt`
    /// coalescing and renders the session as a 1970 timestamp, sinking it to the
    /// bottom of the list.
    #[test]
    fn agent_session_messageless_serializes_last_message_at_as_null() {
        let session = AgentSession {
            id: "agent_session_2".to_string(),
            name: "Fresh Session".to_string(),
            project_id: None,
            agent_definition_id: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: None,
            enabled_tools: Vec::new(),
            mcp_servers: Vec::new(),
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            created_at: 1000,
            updated_at: 1000,
        };

        let json = serde_json::to_string(&session).expect("serialize");
        assert!(
            json.contains("\"lastMessageAt\":null"),
            "messageless session must carry lastMessageAt: null on the wire, got: {}",
            json
        );
        assert!(!json.contains("\"lastMessageAt\":0"));

        // The other nullable fields likewise carry true null, not "".
        assert!(json.contains("\"modelId\":null"));
        assert!(json.contains("\"projectId\":null"));
        assert!(json.contains("\"workingDir\":null"));
        // No enabledSkills key on the wire at all — not even an empty [].
        assert!(!json.contains("\"enabledSkills\""));

        let deserialized: AgentSession = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.last_message_at, None);
        assert_eq!(deserialized.model_id, None);
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

    /// A create/update request still carrying the deprecated enabledSkills key
    /// deserializes fine — serde ignores unknown keys, so old frontends keep working.
    #[test]
    fn requests_ignore_deprecated_enabled_skills_key() {
        let json = r#"{"name": "Test Session", "enabledSkills": ["pdf", "csv"]}"#;
        let req: CreateAgentSessionRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "Test Session");

        let json = r#"{"name": "Renamed", "enabledSkills": []}"#;
        let req: UpdateAgentSessionRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, Some("Renamed".to_string()));
    }
}
