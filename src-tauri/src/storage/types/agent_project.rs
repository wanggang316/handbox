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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_project_serialization_roundtrip() {
        let project = AgentProject {
            id: "agent_project_1".to_string(),
            path: "/tmp/workspace/demo".to_string(),
            name: "demo".to_string(),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&project).expect("serialize");
        // Verify camelCase field naming on the wire.
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        assert!(json.contains("\"path\""));

        let deserialized: AgentProject = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(project.id, deserialized.id);
        assert_eq!(project.path, deserialized.path);
        assert_eq!(project.name, deserialized.name);
        assert_eq!(project.created_at, deserialized.created_at);
        assert_eq!(project.updated_at, deserialized.updated_at);
    }

    #[test]
    fn create_agent_project_request_deserialize() {
        let json = r#"{"path": "/tmp/workspace/demo", "name": "demo"}"#;
        let req: CreateAgentProjectRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.path, "/tmp/workspace/demo");
        assert_eq!(req.name, "demo");
    }
}
