use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Agent, AgentReasoningConfig, McpServerConfig, UUID};
use crate::storage::AgentRepository;
use std::sync::Arc;

pub enum AgentParameter {
    Name(String),
    Temperature(Option<f32>),
    TopP(Option<f32>),
    TopK(Option<i32>),
    MaxTokens(Option<i32>),
    Reasoning(Option<AgentReasoningConfig>),
    SystemPrompt(Option<String>),
    McpServers(Vec<McpServerConfig>),
    Skills(Vec<String>),
    GenerativeUi(Option<bool>),
    GenUiId(Option<UUID>),
    ProviderId(Option<String>),
    Icon(Option<String>),
    Description(Option<String>),
    BuiltinTools(Vec<String>),
    WorkingDirMode(Option<String>),
    ToolExecutionMode(Option<String>),
    ThinkingLevel(Option<String>),
    Starters(Vec<String>),
}

#[derive(Clone)]
pub struct AgentService {
    repository: AgentRepository,
}

impl AgentService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: AgentRepository::new(db),
        }
    }

    // Arguments map 1:1 onto the editable Agent fields; bundling them into a
    // struct would move the parameter list, not shorten it.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_agent(
        &self,
        name: String,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        reasoning: Option<AgentReasoningConfig>,
        max_tokens: Option<i32>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<McpServerConfig>>,
        skills: Option<Vec<String>>,
        generative_ui: Option<bool>,
        genui_id: Option<UUID>,
    ) -> Result<Agent, AppError> {
        let now = Self::current_timestamp();

        let agent = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            temperature,
            top_p,
            top_k,
            reasoning,
            max_tokens,
            system_prompt,
            mcp_servers: mcp_servers.unwrap_or_default(),
            skills: skills.unwrap_or_default(),
            generative_ui,
            genui_id,
            // Extended AgentDefinition fields start empty here and are filled in
            // field by field via `agent_update_field`. User-created is never builtin.
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

        self.repository.create_agent(&agent).await?;
        Ok(agent)
    }

    pub async fn list_agents(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Agent>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        self.repository.list_agents(limit, offset).await
    }

    pub async fn get_agent(&self, agent_id: UUID) -> Result<Agent, AppError> {
        match self.repository.get_agent_by_id(&agent_id).await? {
            Some(agent) => Ok(agent),
            None => Err(AppError::not_found(&format!("Agent not found: {}", agent_id))),
        }
    }

    /// Single entry point for updating one Agent field.
    pub async fn update_agent_parameter(
        &self,
        agent_id: UUID,
        parameter: AgentParameter,
    ) -> Result<Agent, AppError> {
        let mut agent = self.get_agent(agent_id).await?;

        // Builtin definitions keep a fixed display name; other fields stay editable.
        if agent.builtin && matches!(parameter, AgentParameter::Name(_)) {
            return Err(AppError::validation_error("Builtin agent cannot be renamed"));
        }

        match parameter {
            AgentParameter::Name(name) => agent.name = name,
            AgentParameter::Temperature(temp) => agent.temperature = temp,
            AgentParameter::TopP(top_p) => agent.top_p = top_p,
            AgentParameter::TopK(top_k) => agent.top_k = top_k,
            AgentParameter::MaxTokens(max_tokens) => agent.max_tokens = max_tokens,
            AgentParameter::Reasoning(reasoning) => agent.reasoning = reasoning,
            AgentParameter::SystemPrompt(prompt) => agent.system_prompt = prompt,
            AgentParameter::McpServers(servers) => agent.mcp_servers = servers,
            AgentParameter::Skills(skills) => agent.skills = skills,
            AgentParameter::GenerativeUi(v) => agent.generative_ui = v,
            AgentParameter::GenUiId(v) => agent.genui_id = v,
            AgentParameter::ProviderId(v) => agent.provider_id = v,
            AgentParameter::Icon(v) => agent.icon = v,
            AgentParameter::Description(v) => agent.description = v,
            AgentParameter::BuiltinTools(v) => agent.builtin_tools = v,
            AgentParameter::WorkingDirMode(v) => agent.working_dir_mode = v,
            AgentParameter::ToolExecutionMode(v) => agent.tool_execution_mode = v,
            AgentParameter::ThinkingLevel(v) => agent.thinking_level = v,
            AgentParameter::Starters(v) => agent.starters = v,
        }

        agent.updated_at = Self::current_timestamp();
        self.repository.update_agent(&agent).await?;
        Ok(agent)
    }

    /// Batch update of Agent settings. `generative_ui` is deliberately absent:
    /// it is updated only through
    /// `update_agent_parameter(AgentParameter::GenerativeUi)`.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_agent(
        &self,
        agent_id: UUID,
        name: Option<String>,
        temperature: Option<Option<f32>>,
        top_p: Option<Option<f32>>,
        top_k: Option<Option<i32>>,
        reasoning: Option<Option<AgentReasoningConfig>>,
        max_tokens: Option<Option<i32>>,
        system_prompt: Option<Option<String>>,
        mcp_servers: Option<Vec<McpServerConfig>>,
        skills: Option<Vec<String>>,
    ) -> Result<Agent, AppError> {
        let mut agent = self.get_agent(agent_id).await?;

        if let Some(n) = name {
            agent.name = n;
        }
        if let Some(t) = temperature {
            agent.temperature = t;
        }
        if let Some(tp) = top_p {
            agent.top_p = tp;
        }
        if let Some(tk) = top_k {
            agent.top_k = tk;
        }
        if let Some(r) = reasoning {
            agent.reasoning = r;
        }
        if let Some(mt) = max_tokens {
            agent.max_tokens = mt;
        }
        if let Some(sp) = system_prompt {
            agent.system_prompt = sp;
        }
        if let Some(ms) = mcp_servers {
            agent.mcp_servers = ms;
        }
        if let Some(sk) = skills {
            agent.skills = sk;
        }

        agent.updated_at = Self::current_timestamp();
        self.repository.update_agent(&agent).await?;
        Ok(agent)
    }

    pub async fn delete_agent(&self, agent_id: UUID) -> Result<(), AppError> {
        let agent = self.get_agent(agent_id.clone()).await?;

        // Builtin definitions (builtin-chat / builtin-coding) are protected.
        if agent.builtin {
            return Err(AppError::validation_error("Builtin agent cannot be deleted"));
        }

        self.repository.delete_agent(&agent_id).await
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        )
    }

    #[tokio::test]
    async fn creates_service_successfully() {
        let db = create_test_database().await;
        let _service = AgentService::new(db);
    }

    #[tokio::test]
    async fn creates_agent_with_all_fields() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let agent = service
            .create_agent(
                "Code Assistant".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                None,
                Some(2048),
                Some("You are a helpful coding assistant.".to_string()),
                Some(vec![McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec!["tool1".to_string()],
                }]),
                Some(vec!["code-analysis".to_string(), "refactoring".to_string()]),
                None,
                None,
            )
            .await
            .expect("agent creation failed");

        assert_eq!(agent.name, "Code Assistant");
        assert_eq!(agent.temperature, Some(0.7));
        assert_eq!(agent.top_p, Some(0.9));
        assert_eq!(agent.top_k, Some(40));
        assert_eq!(agent.max_tokens, Some(2048));
        assert_eq!(
            agent.system_prompt,
            Some("You are a helpful coding assistant.".to_string())
        );
        assert_eq!(
            agent.mcp_servers,
            vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }]
        );
        assert_eq!(
            agent.skills,
            vec!["code-analysis".to_string(), "refactoring".to_string()]
        );
    }

    #[tokio::test]
    async fn lists_agents_sorted_by_updated_at() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        service
            .create_agent(
                "Agent 1".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        service
            .create_agent(
                "Agent 2".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Exclude the seeded builtin definitions: their updated_at is older, so
        // they sort after the two agents created here.
        let agents: Vec<_> = service
            .list_agents(Some(10), Some(0))
            .await
            .expect("list agents failed")
            .into_iter()
            .filter(|a| !a.builtin)
            .collect();

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "Agent 2");
        assert_eq!(agents[1].name, "Agent 1");
    }

    #[tokio::test]
    async fn fetches_agent_by_id() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Test Agent".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let fetched = service
            .get_agent(created.id.clone())
            .await
            .expect("expected agent");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test Agent");
    }

    #[tokio::test]
    async fn get_agent_returns_not_found_error() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let err = service
            .get_agent("nonexistent_agent".to_string())
            .await
            .expect_err("expected error");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn updates_existing_agent() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Original Name".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let updated = service
            .update_agent(
                created.id.clone(),
                Some("Updated Name".to_string()),
                Some(Some(0.8)),
                Some(Some(0.95)),
                Some(Some(40)),
                None,
                Some(Some(4096)),
                Some(Some("Updated prompt".to_string())),
                Some(vec![
                    McpServerConfig {
                        server_id: "server1".to_string(),
                        execution_mode: "auto".to_string(),
                        enabled_tools: vec!["tool1".to_string(), "tool2".to_string()],
                    },
                    McpServerConfig {
                        server_id: "server2".to_string(),
                        execution_mode: "manual".to_string(),
                        enabled_tools: vec!["tool3".to_string()],
                    },
                ]),
                Some(vec!["skill1".to_string(), "skill2".to_string()]),
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.8));
        assert_eq!(updated.top_p, Some(0.95));
        assert_eq!(updated.top_k, Some(40));
        assert_eq!(updated.max_tokens, Some(4096));
        assert_eq!(updated.system_prompt, Some("Updated prompt".to_string()));
        assert_eq!(updated.skills, vec!["skill1".to_string(), "skill2".to_string()]);
    }

    #[tokio::test]
    async fn delete_agent_removes_record() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "To Delete".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        service
            .delete_agent(created.id.clone())
            .await
            .expect("delete failed");

        let err = service
            .get_agent(created.id)
            .await
            .expect_err("expected missing agent");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn updates_agent_parameter() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Test Agent".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let updated = service
            .update_agent_parameter(created.id.clone(), AgentParameter::Name("New Name".to_string()))
            .await
            .expect("update parameter failed");

        assert_eq!(updated.name, "New Name");

        let updated_temp = service
            .update_agent_parameter(created.id.clone(), AgentParameter::Temperature(Some(0.5)))
            .await
            .expect("update parameter failed");

        assert_eq!(updated_temp.temperature, Some(0.5));

        let updated_skills = service
            .update_agent_parameter(
                created.id.clone(),
                AgentParameter::Skills(vec!["skill1".to_string(), "skill2".to_string()]),
            )
            .await
            .expect("update parameter failed");

        assert_eq!(
            updated_skills.skills,
            vec!["skill1".to_string(), "skill2".to_string()]
        );
    }

    #[tokio::test]
    async fn clears_parameters_when_passed_some_none() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Test Agent".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                None,
                Some(2048),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(created.temperature, Some(0.7));
        assert_eq!(created.top_p, Some(0.9));
        assert_eq!(created.top_k, Some(40));
        assert_eq!(created.max_tokens, Some(2048));

        let updated = service
            .update_agent(
                created.id.clone(),
                None,
                Some(None), // clear temperature
                Some(None), // clear top_p
                Some(None), // clear top_k
                None,
                Some(None), // clear max_tokens
                None,
                None,
                None,
            )
            .await
            .expect("update failed");

        assert_eq!(updated.temperature, None);
        assert_eq!(updated.top_p, None);
        assert_eq!(updated.top_k, None);
        assert_eq!(updated.max_tokens, None);
    }

    #[tokio::test]
    async fn preserves_parameters_when_passed_none() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Test Agent".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                None,
                Some(2048),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let updated = service
            .update_agent(
                created.id.clone(),
                Some("Updated Name".to_string()),
                None, // leave temperature untouched
                None, // leave top_p untouched
                None, // leave top_k untouched
                None,
                None, // leave max_tokens untouched
                None,
                None,
                None,
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.7));
        assert_eq!(updated.top_p, Some(0.9));
        assert_eq!(updated.top_k, Some(40));
        assert_eq!(updated.max_tokens, Some(2048));
    }

    /// Creating an agent with generative_ui = Some(true) persists and reads
    /// back `true`.
    #[tokio::test]
    async fn creates_agent_with_generative_ui_true() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Generative Agent".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                None,
            )
            .await
            .unwrap();

        assert_eq!(created.generative_ui, Some(true));

        let fetched = service.get_agent(created.id).await.unwrap();
        assert_eq!(fetched.generative_ui, Some(true));
    }

    /// Turning generative_ui OFF must persist `false`, not be swallowed as a
    /// falsy value at the service layer.
    #[tokio::test]
    async fn update_generative_ui_false_persists() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Generative Agent".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(true),
                None,
            )
            .await
            .unwrap();
        assert_eq!(created.generative_ui, Some(true));

        let updated = service
            .update_agent_parameter(
                created.id.clone(),
                AgentParameter::GenerativeUi(Some(false)),
            )
            .await
            .expect("update generative_ui failed");
        assert_eq!(updated.generative_ui, Some(false));

        let fetched = service.get_agent(created.id).await.unwrap();
        assert_eq!(fetched.generative_ui, Some(false));
    }

    /// Editing one field (Name) must preserve every other field.
    #[tokio::test]
    async fn preserves_generative_ui_on_unrelated_edit() {
        let db = create_test_database().await;
        let service = AgentService::new(db);

        let created = service
            .create_agent(
                "Original Name".to_string(),
                Some(0.7),
                None,
                None,
                None,
                None,
                Some("Original prompt".to_string()),
                None,
                Some(vec!["skill1".to_string()]),
                Some(true),
                None,
            )
            .await
            .unwrap();

        let updated = service
            .update_agent_parameter(
                created.id.clone(),
                AgentParameter::Name("Renamed".to_string()),
            )
            .await
            .expect("rename failed");

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.generative_ui, Some(true)); // preserved
        assert_eq!(updated.system_prompt, Some("Original prompt".to_string()));
        assert_eq!(updated.skills, vec!["skill1".to_string()]);
        assert_eq!(updated.temperature, Some(0.7));
    }
}
