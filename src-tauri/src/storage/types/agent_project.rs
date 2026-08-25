use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// Groups Agent-mode sessions by working directory.
///
/// `path` is the canonicalized working directory (canonicalization happens in
/// the service layer; the repository dedupes by exact string equality) and is
/// UNIQUE at the database level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProject {
    pub id: UUID,
    pub path: String,
    pub name: String,
    /// Floats the project to the top of the sidebar's project section.
    pub pinned: bool,
    /// `#rrggbb` tint for the sidebar row's name; `None` = the default color.
    pub color: Option<String>,
    /// Per-project "Open in ..." target id; `None` = follow the global default.
    pub default_editor_id: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Get-or-create semantics: the existing project is returned for a known path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentProjectRequest {
    pub path: String,
    pub name: String,
}

/// The project settings panel's fields, written as one group.
///
/// Whole-group replacement rather than a patch: `color` / `default_editor_id`
/// have "unset" as a real value, and under patch semantics a missing field
/// would be indistinguishable from an explicit clear. `pinned` is deliberately
/// absent — it is toggled from the sidebar menu through its own single-column
/// write, so a settings save can never clobber a concurrent pin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentProjectSettingsRequest {
    pub name: String,
    pub color: Option<String>,
    pub default_editor_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_project_serialization_roundtrip() {
        let project = AgentProject {
            id: "agent_project_1".to_string(),
            path: "/tmp/workspace/demo".to_string(),
            name: "demo".to_string(),
            pinned: true,
            color: Some("#ff3b30".to_string()),
            default_editor_id: Some("zed".to_string()),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&project).expect("serialize");
        // Verify camelCase field naming on the wire.
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        assert!(json.contains("\"path\""));
        assert!(json.contains("\"defaultEditorId\""));

        let deserialized: AgentProject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(project.id, deserialized.id);
        assert_eq!(project.path, deserialized.path);
        assert_eq!(project.name, deserialized.name);
        assert_eq!(project.pinned, deserialized.pinned);
        assert_eq!(project.color, deserialized.color);
        assert_eq!(project.default_editor_id, deserialized.default_editor_id);
        assert_eq!(project.created_at, deserialized.created_at);
        assert_eq!(project.updated_at, deserialized.updated_at);
    }

    /// The settings request's optional fields accept an explicit null, which is
    /// how the panel clears the color / falls back to the global editor.
    #[test]
    fn update_agent_project_settings_request_deserialize() {
        let json = r#"{"name": "demo", "color": null, "defaultEditorId": "zed"}"#;
        let req: UpdateAgentProjectSettingsRequest =
            serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name, "demo");
        assert_eq!(req.color, None);
        assert_eq!(req.default_editor_id.as_deref(), Some("zed"));
    }

    #[test]
    fn create_agent_project_request_deserialize() {
        let json = r#"{"path": "/tmp/workspace/demo", "name": "demo"}"#;
        let req: CreateAgentProjectRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.path, "/tmp/workspace/demo");
        assert_eq!(req.name, "demo");
    }
}
