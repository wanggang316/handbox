// Agent 服务实现

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{Agent, AgentReasoningConfig, McpServerConfig, UUID};
use crate::storage::AgentRepository;
use std::sync::Arc;

/// Agent 参数类型
pub enum AgentParameter {
    Name(String),
    Model(String),
    Temperature(Option<f32>),
    TopP(Option<f32>),
    TopK(Option<i32>),
    MaxTokens(Option<i32>),
    Reasoning(Option<AgentReasoningConfig>),
    SystemPrompt(Option<String>),
    McpServers(Vec<McpServerConfig>),
    Skills(Vec<String>),
}

/// Agent 服务
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

    /// 创建 Agent
    pub async fn create_agent(
        &self,
        name: String,
        model: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        reasoning: Option<AgentReasoningConfig>,
        max_tokens: Option<i32>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<McpServerConfig>>,
        skills: Option<Vec<String>>,
    ) -> Result<Agent, AppError> {
        let now = Self::current_timestamp();

        let agent = Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            model,
            temperature,
            top_p,
            top_k,
            reasoning,
            max_tokens,
            system_prompt,
            mcp_servers: mcp_servers.unwrap_or_default(),
            skills: skills.unwrap_or_default(),
            generative_ui: None,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_agent(&agent).await?;
        Ok(agent)
    }

    /// 获取 Agent 列表
    pub async fn list_agents(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Agent>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        self.repository.list_agents(limit, offset).await
    }

    /// 获取 Agent 详情
    pub async fn get_agent(&self, agent_id: UUID) -> Result<Agent, AppError> {
        match self.repository.get_agent_by_id(&agent_id).await? {
            Some(agent) => Ok(agent),
            None => Err(AppError::not_found(&format!("Agent not found: {}", agent_id))),
        }
    }

    /// 统一的参数更新方法
    pub async fn update_agent_parameter(
        &self,
        agent_id: UUID,
        parameter: AgentParameter,
    ) -> Result<Agent, AppError> {
        let mut agent = self.get_agent(agent_id).await?;

        match parameter {
            AgentParameter::Name(name) => agent.name = name,
            AgentParameter::Model(model) => agent.model = Some(model),
            AgentParameter::Temperature(temp) => agent.temperature = temp,
            AgentParameter::TopP(top_p) => agent.top_p = top_p,
            AgentParameter::TopK(top_k) => agent.top_k = top_k,
            AgentParameter::MaxTokens(max_tokens) => agent.max_tokens = max_tokens,
            AgentParameter::Reasoning(reasoning) => agent.reasoning = reasoning,
            AgentParameter::SystemPrompt(prompt) => agent.system_prompt = prompt,
            AgentParameter::McpServers(servers) => agent.mcp_servers = servers,
            AgentParameter::Skills(skills) => agent.skills = skills,
        }

        agent.updated_at = Self::current_timestamp();
        self.repository.update_agent(&agent).await?;
        Ok(agent)
    }

    /// 批量更新 Agent 设置
    pub async fn update_agent(
        &self,
        agent_id: UUID,
        name: Option<String>,
        model: Option<String>,
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
        if let Some(m) = model {
            agent.model = Some(m);
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

    /// 删除 Agent
    pub async fn delete_agent(&self, agent_id: UUID) -> Result<(), AppError> {
        // 先检查 Agent 是否存在
        self.get_agent(agent_id.clone()).await?;

        // 删除 Agent
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
                Some("gpt-4o".to_string()),
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
            )
            .await
            .expect("agent creation failed");

        assert_eq!(agent.name, "Code Assistant");
        assert_eq!(agent.model, Some("gpt-4o".to_string()));
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
            )
            .await
            .unwrap();

        let agents = service
            .list_agents(Some(10), Some(0))
            .await
            .expect("list agents failed");

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
            )
            .await
            .unwrap();

        let updated = service
            .update_agent(
                created.id.clone(),
                Some("Updated Name".to_string()),
                Some("gpt-4o".to_string()),
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
        assert_eq!(updated.model, Some("gpt-4o".to_string()));
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
                Some("gpt-4o".to_string()),
                Some(0.7),
                Some(0.9),
                Some(40),
                None,
                Some(2048),
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
                None,
                Some(None), // 清空 temperature
                Some(None), // 清空 top_p
                Some(None), // 清空 top_k
                None,
                Some(None), // 清空 max_tokens
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
                Some("gpt-4o".to_string()),
                Some(0.7),
                Some(0.9),
                Some(40),
                None,
                Some(2048),
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
                None,
                None, // 不修改 temperature，保持原值
                None, // 不修改 top_p，保持原值
                None, // 不修改 top_k，保持原值
                None,
                None, // 不修改 max_tokens，保持原值
                None,
                None,
                None,
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.7)); // 保持原值
        assert_eq!(updated.top_p, Some(0.9)); // 保持原值
        assert_eq!(updated.top_k, Some(40)); // 保持原值
        assert_eq!(updated.max_tokens, Some(2048)); // 保持原值
    }
}
