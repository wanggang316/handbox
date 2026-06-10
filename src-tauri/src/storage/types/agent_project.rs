use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// Agent Project 实体 - 按工作目录分组 Agent 模式会话
///
/// `path` 为 canonical 化后的工作目录（canonicalize 在 service 层完成，
/// 仓库层按字符串全等去重），数据库层有 UNIQUE 约束。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProject {
    pub id: UUID,
    pub path: String,
    pub name: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 创建 Agent Project 请求（get-or-create 语义：同 path 返回已有项目）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentProjectRequest {
    pub path: String,
    pub name: String,
}

/// 更新 Agent Project 请求（path 不可变，仅 name 可更新）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentProjectRequest {
    pub name: Option<String>,
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

    #[test]
    fn update_agent_project_request_partial() {
        let json = r#"{}"#;
        let req: UpdateAgentProjectRequest = serde_json::from_str(json).expect("deserialize");
        assert!(req.name.is_none());

        let json = r#"{"name": "renamed"}"#;
        let req: UpdateAgentProjectRequest = serde_json::from_str(json).expect("deserialize");
        assert_eq!(req.name.as_deref(), Some("renamed"));
    }
}
