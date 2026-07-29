use crate::models::AppError;
use crate::storage::types::{Agent, McpServerConfig, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

#[derive(Clone)]
pub struct AgentRepository {
    db: Arc<Database>,
}

impl AgentRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn create_agent(&self, agent: &Agent) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&agent.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let reasoning_json = agent
            .reasoning
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok());

        let skills_json = serde_json::to_string(&agent.skills)
            .map_err(|e| AppError::validation_error(&format!("Invalid skills: {}", e)))?;

        let builtin_tools_json = serde_json::to_string(&agent.builtin_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid builtin tools: {}", e)))?;

        let starters_json = serde_json::to_string(&agent.starters)
            .map_err(|e| AppError::validation_error(&format!("Invalid starters: {}", e)))?;

        let query = r#"
            INSERT INTO agents (id, name, temperature, top_p, top_k, reasoning, max_tokens, system_prompt, mcp_servers, skills, generative_ui, genui_id, provider_id, icon, description, builtin, builtin_tools, working_dir_mode, tool_execution_mode, thinking_level, starters, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23)
        "#;

        sqlx::query(query)
            .bind(&agent.id)
            .bind(&agent.name)
            .bind(agent.temperature)
            .bind(agent.top_p)
            .bind(agent.top_k)
            .bind(reasoning_json)
            .bind(agent.max_tokens)
            .bind(&agent.system_prompt)
            .bind(&mcp_servers_json)
            .bind(&skills_json)
            .bind(agent.generative_ui)
            .bind(&agent.genui_id)
            .bind(&agent.provider_id)
            .bind(&agent.icon)
            .bind(&agent.description)
            .bind(agent.builtin)
            .bind(&builtin_tools_json)
            .bind(&agent.working_dir_mode)
            .bind(&agent.tool_execution_mode)
            .bind(&agent.thinking_level)
            .bind(&starters_json)
            .bind(agent.created_at)
            .bind(agent.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create agent: {}", e)))?;

        Ok(())
    }

    pub async fn list_agents(&self, limit: i32, offset: i32) -> Result<Vec<Agent>, AppError> {
        let query = r#"
            SELECT id, name, temperature, top_p, top_k, reasoning, max_tokens, system_prompt, mcp_servers, skills, generative_ui, genui_id, provider_id, icon, description, builtin, builtin_tools, working_dir_mode, tool_execution_mode, thinking_level, starters, created_at, updated_at
            FROM agents ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list agents: {}", e)))?;

        let mut agents = Vec::new();
        for row in rows {
            agents.push(self.row_to_agent(row)?);
        }

        Ok(agents)
    }

    pub async fn get_agent_by_id(&self, agent_id: &UUID) -> Result<Option<Agent>, AppError> {
        let query = r#"
            SELECT id, name, temperature, top_p, top_k, reasoning, max_tokens, system_prompt, mcp_servers, skills, generative_ui, genui_id, provider_id, icon, description, builtin, builtin_tools, working_dir_mode, tool_execution_mode, thinking_level, starters, created_at, updated_at
            FROM agents WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(agent_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get agent: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_agent(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_agent(&self, agent: &Agent) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&agent.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let reasoning_json = agent
            .reasoning
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok());

        let skills_json = serde_json::to_string(&agent.skills)
            .map_err(|e| AppError::validation_error(&format!("Invalid skills: {}", e)))?;

        let builtin_tools_json = serde_json::to_string(&agent.builtin_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid builtin tools: {}", e)))?;

        let starters_json = serde_json::to_string(&agent.starters)
            .map_err(|e| AppError::validation_error(&format!("Invalid starters: {}", e)))?;

        let query = r#"
            UPDATE agents SET name = $1, temperature = $2, top_p = $3, top_k = $4, reasoning = $5, max_tokens = $6, system_prompt = $7, mcp_servers = $8, skills = $9, generative_ui = $10, genui_id = $11, provider_id = $12, icon = $13, description = $14, builtin = $15, builtin_tools = $16, working_dir_mode = $17, tool_execution_mode = $18, thinking_level = $19, starters = $20, updated_at = $21
            WHERE id = $22
        "#;

        let result = sqlx::query(query)
            .bind(&agent.name)
            .bind(agent.temperature)
            .bind(agent.top_p)
            .bind(agent.top_k)
            .bind(reasoning_json)
            .bind(agent.max_tokens)
            .bind(&agent.system_prompt)
            .bind(&mcp_servers_json)
            .bind(&skills_json)
            .bind(agent.generative_ui)
            .bind(&agent.genui_id)
            .bind(&agent.provider_id)
            .bind(&agent.icon)
            .bind(&agent.description)
            .bind(agent.builtin)
            .bind(&builtin_tools_json)
            .bind(&agent.working_dir_mode)
            .bind(&agent.tool_execution_mode)
            .bind(&agent.thinking_level)
            .bind(&starters_json)
            .bind(agent.updated_at)
            .bind(&agent.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update agent: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Agent not found: {}", agent.id)));
        }

        Ok(())
    }

    pub async fn delete_agent(&self, agent_id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM agents WHERE id = $1")
            .bind(agent_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete agent: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Agent not found: {}", agent_id)));
        }

        Ok(())
    }

    /// Removes references to the given MCP server from all agents; returns the
    /// number of agents updated.
    pub async fn remove_mcp_server_from_agents(&self, server_id: &str) -> Result<i32, AppError> {
        // Coarse LIKE prefilter; exact matching happens via `retain` below.
        let query = r#"
            SELECT id, mcp_servers
            FROM agents
            WHERE mcp_servers LIKE '%' || $1 || '%'
        "#;

        let rows = sqlx::query(query)
            .bind(server_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to query agents with MCP server: {}", e))
            })?;

        let mut updated_count = 0;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for row in rows {
            let agent_id: String = row.try_get("id")?;
            let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;

            if let Some(json) = mcp_servers_json {
                let mut mcp_servers: Vec<McpServerConfig> =
                    serde_json::from_str(&json).unwrap_or_default();

                let original_len = mcp_servers.len();
                mcp_servers.retain(|config| config.server_id != server_id);

                if mcp_servers.len() < original_len {
                    let updated_json = serde_json::to_string(&mcp_servers).map_err(|e| {
                        AppError::internal_error(&format!("Failed to serialize MCP servers: {}", e))
                    })?;

                    let update_query = r#"
                        UPDATE agents
                        SET mcp_servers = $1, updated_at = $2
                        WHERE id = $3
                    "#;

                    sqlx::query(update_query)
                        .bind(&updated_json)
                        .bind(now)
                        .bind(&agent_id)
                        .execute(self.db.pool())
                        .await
                        .map_err(|e| {
                            AppError::internal_error(&format!(
                                "Failed to update agent MCP servers: {}",
                                e
                            ))
                        })?;

                    updated_count += 1;
                }
            }
        }

        Ok(updated_count)
    }

    fn row_to_agent(&self, row: sqlx::sqlite::SqliteRow) -> Result<Agent, AppError> {
        let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;
        let mcp_servers: Vec<McpServerConfig> = if let Some(json) = mcp_servers_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let reasoning = row
            .try_get::<Option<String>, _>("reasoning")?
            .and_then(|raw| match raw.trim() {
                "" => None,
                value => serde_json::from_str(value).ok(),
            });

        let skills_json: Option<String> = row.try_get("skills")?;
        let skills: Vec<String> = if let Some(json) = skills_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let temperature: Option<f32> = row.try_get::<Option<f32>, _>("temperature")?;
        let top_p: Option<f32> = row.try_get::<Option<f32>, _>("top_p")?;
        let top_k: Option<i32> = row.try_get::<Option<i32>, _>("top_k")?;
        let max_tokens: Option<i32> = row.try_get::<Option<i32>, _>("max_tokens")?;
        // Decode as Option: SQL NULL (legacy rows) -> None, INTEGER 0/1 -> Some.
        let generative_ui: Option<bool> = row.try_get::<Option<bool>, _>("generative_ui")?;
        // SQL NULL -> None (legacy rows / unlinked).
        let genui_id: Option<String> = row.try_get::<Option<String>, _>("genui_id")?;

        // AgentDefinition columns are all decoded NULL-safe for legacy rows.
        let provider_id: Option<String> = row.try_get::<Option<String>, _>("provider_id")?;
        let icon: Option<String> = row.try_get::<Option<String>, _>("icon")?;
        let description: Option<String> = row.try_get::<Option<String>, _>("description")?;
        // builtin: SQL NULL (legacy rows) -> false.
        let builtin: bool = row.try_get::<Option<bool>, _>("builtin")?.unwrap_or(false);
        let builtin_tools_json: Option<String> = row.try_get("builtin_tools")?;
        let builtin_tools: Vec<String> = if let Some(json) = builtin_tools_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };
        let working_dir_mode: Option<String> =
            row.try_get::<Option<String>, _>("working_dir_mode")?;
        let tool_execution_mode: Option<String> =
            row.try_get::<Option<String>, _>("tool_execution_mode")?;
        let thinking_level: Option<String> = row.try_get::<Option<String>, _>("thinking_level")?;
        let starters_json: Option<String> = row.try_get("starters")?;
        let starters: Vec<String> = if let Some(json) = starters_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Agent {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            temperature,
            top_p,
            top_k,
            reasoning,
            max_tokens,
            system_prompt: row.try_get("system_prompt").ok(),
            mcp_servers,
            skills,
            generative_ui,
            genui_id,
            provider_id,
            icon,
            description,
            builtin,
            builtin_tools,
            working_dir_mode,
            tool_execution_mode,
            thinking_level,
            starters,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path).await.unwrap();
        (db_service, temp_dir)
    }

    #[tokio::test]
    async fn test_agent_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let agent = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Code Assistant".to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            reasoning: None,
            max_tokens: Some(2048),
            system_prompt: Some("You are a helpful coding assistant.".to_string()),
            mcp_servers: vec![
                McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec!["tool1".to_string()],
                },
                McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "manual".to_string(),
                    enabled_tools: vec![],
                },
            ],
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
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_agent(&agent).await.unwrap();

        // Get by ID
        let fetched = repo.get_agent_by_id(&agent.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched_agent = fetched.unwrap();
        assert_eq!(fetched_agent.name, agent.name);
        assert_eq!(fetched_agent.system_prompt, agent.system_prompt);
        assert_eq!(fetched_agent.mcp_servers, agent.mcp_servers);
        assert_eq!(fetched_agent.skills, agent.skills);

        // List (exclude the builtin AgentDefinitions seeded by migration 058)
        let agents: Vec<_> = repo
            .list_agents(10, 0)
            .await
            .unwrap()
            .into_iter()
            .filter(|a| !a.builtin)
            .collect();
        assert_eq!(agents.len(), 1);

        // Update
        let mut updated_agent = agent.clone();
        updated_agent.name = "Updated Agent".to_string();
        updated_agent.updated_at = now + 1000;

        repo.update_agent(&updated_agent).await.unwrap();

        let fetched_updated = repo.get_agent_by_id(&agent.id).await.unwrap();
        assert_eq!(fetched_updated.unwrap().name, "Updated Agent");

        // Delete
        repo.delete_agent(&agent.id).await.unwrap();
        let deleted = repo.get_agent_by_id(&agent.id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_remove_mcp_server_from_agents() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let agent1 = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Agent 1".to_string(),
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            max_tokens: None,
            system_prompt: None,
            mcp_servers: vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec![],
            }],
            skills: vec![],
            generative_ui: None,
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
            created_at: now,
            updated_at: now,
        };

        let agent2 = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Agent 2".to_string(),
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            max_tokens: None,
            system_prompt: None,
            mcp_servers: vec![
                McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
                McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
            ],
            skills: vec![],
            generative_ui: None,
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
            created_at: now,
            updated_at: now,
        };

        repo.create_agent(&agent1).await.unwrap();
        repo.create_agent(&agent2).await.unwrap();

        let updated_count = repo.remove_mcp_server_from_agents("server1").await.unwrap();
        assert_eq!(updated_count, 2);

        let updated_agent1 = repo.get_agent_by_id(&agent1.id).await.unwrap().unwrap();
        assert_eq!(updated_agent1.mcp_servers.len(), 0);

        let updated_agent2 = repo.get_agent_by_id(&agent2.id).await.unwrap().unwrap();
        assert_eq!(updated_agent2.mcp_servers.len(), 1);
        assert_eq!(updated_agent2.mcp_servers[0].server_id, "server2");
    }

    fn minimal_agent(now: i64, generative_ui: Option<bool>) -> Agent {
        Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name: "GenUI Agent".to_string(),
            temperature: None,
            top_p: None,
            top_k: None,
            reasoning: None,
            max_tokens: None,
            system_prompt: None,
            mcp_servers: vec![],
            skills: vec![],
            generative_ui,
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
            created_at: now,
            updated_at: now,
        }
    }

    // generative_ui round-trips through create -> get -> update -> get.
    #[tokio::test]
    async fn test_agent_generative_ui_round_trip() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let agent = minimal_agent(now, Some(true));
        repo.create_agent(&agent).await.unwrap();

        let fetched = repo.get_agent_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(fetched.generative_ui, Some(true));

        let mut updated = fetched.clone();
        updated.generative_ui = Some(false);
        updated.updated_at = now + 1000;
        repo.update_agent(&updated).await.unwrap();

        let after = repo.get_agent_by_id(&agent.id).await.unwrap().unwrap();
        assert_eq!(after.generative_ui, Some(false));
    }

    // An old row with generative_ui NULL decodes to None on both read paths
    // (get_agent_by_id + list_agents) without a sqlx decode error.
    #[tokio::test]
    async fn test_agent_generative_ui_null_decodes_to_none() {
        let (db, _temp_dir) = create_test_db().await;
        let db = Arc::new(db);
        let repo = AgentRepository::new(db.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Simulate a legacy row: insert directly, omitting generative_ui (stays NULL).
        let agent_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agents (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&agent_id)
            .bind("Legacy Agent")
            .bind(now)
            .bind(now)
            .execute(db.pool())
            .await
            .unwrap();

        let fetched = repo.get_agent_by_id(&agent_id).await.unwrap().unwrap();
        assert_eq!(fetched.generative_ui, None);

        let listed: Vec<_> = repo
            .list_agents(10, 0)
            .await
            .unwrap()
            .into_iter()
            .filter(|a| !a.builtin)
            .collect();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].generative_ui, None);
    }
}
