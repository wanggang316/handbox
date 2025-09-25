// MCP service: manages Model Context Protocol server configurations

use std::collections::HashMap;
use std::{path::PathBuf, sync::Arc};

use crate::mcp_client::McpClient;
use crate::models::{
    AppError, CreateMcpServerRequest, McpServer, McpServerStatus, McpTool, RefreshMcpServerRequest,
    ToggleMcpServerRequest, UpdateMcpServerRequest,
};
use crate::services::Database;
use crate::storage::McpRepository;

/// Service orchestrating MCP server lifecycle and metadata
#[derive(Clone)]
pub struct McpService {
    repository: McpRepository,
}

impl McpService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: McpRepository::new(db),
        }
    }

    /// List all MCP servers
    pub async fn list_servers(&self) -> Result<Vec<McpServer>, AppError> {
        self.repository.list_servers().await
    }

    /// Get a server by id
    pub async fn get_server(&self, id: &str) -> Result<McpServer, AppError> {
        self.repository
            .get_server(id)
            .await?
            .ok_or_else(|| AppError::not_found(&format!("MCP server not found: {}", id)))
    }

    /// Get multiple servers preserving requested order
    pub async fn get_servers_by_ids(&self, ids: &[String]) -> Result<Vec<McpServer>, AppError> {
        self.repository.get_servers_by_ids(ids).await
    }

    /// Create a new MCP server configuration
    pub async fn create_server(
        &self,
        mut request: CreateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        Self::validate_create_request(&request)?;

        Self::normalize_create_request(&mut request);

        let now = Self::current_timestamp();
        let mut server = McpServer {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            display_name: request.display_name,
            description: request.description,
            command: request.command,
            args: request.args,
            working_dir: request.working_dir,
            env: request.env,
            enabled: request.enabled,
            status: if request.enabled {
                McpServerStatus::Ready
            } else {
                McpServerStatus::Inactive
            },
            tools: Vec::new(),
            last_sync_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        };

        if server.enabled {
            self.populate_server_metadata(&mut server).await;
        }

        self.repository.create_server(&server).await?;
        Ok(server)
    }

    /// Update server attributes
    pub async fn update_server(
        &self,
        server_id: String,
        mut request: UpdateMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        let mut existing = self.get_server(&server_id).await?;

        Self::normalize_update_request(&mut request);

        if let Some(name) = request.name.take() {
            if name.is_empty() {
                return Err(AppError::validation_error("MCP 服务器名称不能为空"));
            }
            existing.name = name;
        }
        if request.display_name.is_some() {
            existing.display_name = request.display_name.take();
        }
        if request.description.is_some() {
            existing.description = request.description.take();
        }
        if let Some(command) = request.command.take() {
            if command.is_empty() {
                return Err(AppError::validation_error("MCP 服务器命令不能为空"));
            }
            existing.command = command;
        }
        if let Some(args) = request.args.take() {
            existing.args = args;
        }
        if request.working_dir.is_some() {
            existing.working_dir = request.working_dir.take();
        }
        if let Some(env) = request.env.take() {
            existing.env = env;
        }

        let mut should_refresh = false;
        if let Some(enabled) = request.enabled {
            existing.enabled = enabled;
            should_refresh = enabled;
            existing.status = if enabled {
                McpServerStatus::Ready
            } else {
                McpServerStatus::Inactive
            };
            if !enabled {
                existing.last_error = None;
            }
        }

        existing.updated_at = Self::current_timestamp();

        if should_refresh {
            self.populate_server_metadata(&mut existing).await;
        }

        self.repository.update_server(&existing).await?;
        Ok(existing)
    }

    /// Toggle enabled state
    pub async fn toggle_server(
        &self,
        request: ToggleMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        let mut server = self.get_server(&request.server_id).await?;
        if server.enabled == request.enabled {
            return Ok(server);
        }

        server.enabled = request.enabled;
        server.updated_at = Self::current_timestamp();
        if request.enabled {
            server.status = McpServerStatus::Ready;
            self.populate_server_metadata(&mut server).await;
        } else {
            server.status = McpServerStatus::Inactive;
            server.last_error = None;
        }

        self.repository.update_server(&server).await?;
        Ok(server)
    }

    /// Refresh metadata (tools, status) for a server
    pub async fn refresh_server(
        &self,
        request: RefreshMcpServerRequest,
    ) -> Result<McpServer, AppError> {
        let mut server = self.get_server(&request.server_id).await?;
        self.populate_server_metadata(&mut server).await;
        server.updated_at = Self::current_timestamp();
        self.repository.update_server(&server).await?;
        Ok(server)
    }

    /// Delete a server definition
    pub async fn delete_server(&self, server_id: String) -> Result<(), AppError> {
        self.repository.delete_server(&server_id).await
    }

    /// Attach metadata (tools, status) by querying the MCP server. Errors are captured in status.
    async fn populate_server_metadata(&self, server: &mut McpServer) {
        match self.fetch_server_tools(server).await {
            Ok(tools) => {
                server.tools = tools;
                server.status = McpServerStatus::Ready;
                server.last_error = None;
                server.last_sync_at = Some(Self::current_timestamp());
            }
            Err(error) => {
                tracing::error!(
                    "Failed to fetch MCP metadata for {}: {}",
                    server.name,
                    error
                );
                server.status = McpServerStatus::Error;
                server.last_error = Some(error.to_string());
            }
        }
    }

    fn validate_create_request(request: &CreateMcpServerRequest) -> Result<(), AppError> {
        if request.name.trim().is_empty() {
            return Err(AppError::validation_error("MCP 服务器名称不能为空"));
        }
        if request.command.trim().is_empty() {
            return Err(AppError::validation_error("MCP 服务器命令不能为空"));
        }
        Ok(())
    }

    fn normalize_create_request(request: &mut CreateMcpServerRequest) {
        request.name = request.name.trim().to_string();
        request.display_name = request
            .display_name
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        request.description = request
            .description
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        request.command = request.command.trim().to_string();
        request.args = Self::normalize_args(std::mem::take(&mut request.args));
        request.working_dir = request
            .working_dir
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        request.env = Self::normalize_env(std::mem::take(&mut request.env));
    }

    fn normalize_update_request(request: &mut UpdateMcpServerRequest) {
        if let Some(name) = request.name.as_mut() {
            *name = name.trim().to_string();
        }
        if let Some(display_name) = request.display_name.as_mut() {
            *display_name = display_name.trim().to_string();
            if display_name.is_empty() {
                request.display_name = None;
            }
        }
        if let Some(description) = request.description.as_mut() {
            *description = description.trim().to_string();
            if description.is_empty() {
                request.description = None;
            }
        }
        if let Some(command) = request.command.as_mut() {
            *command = command.trim().to_string();
        }
        if let Some(args) = request.args.as_mut() {
            *args = Self::normalize_args(std::mem::take(args));
        }
        if let Some(working_dir) = request.working_dir.as_mut() {
            *working_dir = working_dir.trim().to_string();
            if working_dir.is_empty() {
                request.working_dir = None;
            }
        }
        if let Some(env) = request.env.as_mut() {
            *env = Self::normalize_env(std::mem::take(env));
        }
    }

    fn normalize_args(args: Vec<String>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.trim().to_string())
            .filter(|arg| !arg.is_empty())
            .collect()
    }

    fn normalize_env(env: HashMap<String, String>) -> HashMap<String, String> {
        env.into_iter()
            .filter_map(|(key, value)| {
                let trimmed_key = key.trim();
                if trimmed_key.is_empty() {
                    return None;
                }
                Some((trimmed_key.to_string(), value))
            })
            .collect()
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    async fn fetch_server_tools(&self, server: &McpServer) -> anyhow::Result<Vec<McpTool>> {
        let working_dir = server.working_dir.as_ref().map(PathBuf::from);

        let client = McpClient::connect_process(
            &server.command,
            &server.args,
            working_dir.as_deref(),
            &server.env,
        )
        .await?;

        let tools = client.list_tools().await?;
        client.shutdown().await.ok();

        Ok(tools)
    }
}
