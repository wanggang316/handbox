// MCP service: manages Model Context Protocol server configurations

use std::{collections::HashMap, sync::Arc};

use crate::models::{
    AppError, CreateMcpServerRequest, RefreshMcpServerRequest, ToggleMcpServerRequest,
    UpdateMcpServerRequest, UpdateToolEnabledRequest,
};
use crate::services::Database;
use crate::storage::types::{McpServer, McpServerStatus};
use crate::storage::{ChatRepository, McpRepository};
use handbox_mcp::{
    validate_server_config, ConnectionConfig, McpClient, McpClientError, McpPrompt,
    McpPromptArgument, McpResource, McpTool, ProcessConfig, SseConfig, StreamableHttpConfig,
};

/// Service orchestrating MCP server lifecycle and metadata
#[derive(Clone)]
pub struct McpService {
    repository: McpRepository,
    chat_repository: ChatRepository,
}

impl McpService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: McpRepository::new(db.clone()),
            chat_repository: ChatRepository::new(db),
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
            connection_type: request.connection_type,
            command: request.command,
            args: request.args,
            working_dir: request.working_dir,
            env: request.env,
            endpoint: request.endpoint,
            headers: request.headers,
            timeout_ms: request.timeout_ms,
            enabled: request.enabled,
            status: if request.enabled {
                McpServerStatus::Ready
            } else {
                McpServerStatus::Inactive
            },
            tools: Vec::new(),
            prompts: Vec::new(),
            resources: Vec::new(),
            enabled_tools: Vec::new(),
            last_sync_at: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        };

        if server.enabled {
            // 如果启用，尝试连接。连接失败则返回错误，不保存
            self.update_server_status(&mut server)
                .await
                .map_err(|e| AppError::internal_error(&e.to_string()))?;
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
            // 只有 stdio 连接类型才需要验证 command 不为空
            if existing.connection_type == crate::storage::types::McpConnectionType::Stdio
                && command.is_empty()
            {
                return Err(AppError::validation_error("stdio 连接类型需要命令不能为空"));
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
        if let Some(connection_type) = request.connection_type.take() {
            existing.connection_type = connection_type;
        }
        if request.endpoint.is_some() {
            existing.endpoint = request.endpoint.take();
        }
        if let Some(headers) = request.headers.take() {
            existing.headers = headers;
        }
        if let Some(timeout_ms) = request.timeout_ms {
            existing.timeout_ms = Some(timeout_ms);
        }

        // 更新完字段后，验证配置的完整性
        match existing.connection_type {
            crate::storage::types::McpConnectionType::Stdio => {
                if existing.command.trim().is_empty() {
                    return Err(AppError::validation_error("stdio 连接类型需要命令不能为空"));
                }
            }
            crate::storage::types::McpConnectionType::Sse
            | crate::storage::types::McpConnectionType::Http => {
                if let Some(ref endpoint) = existing.endpoint {
                    if endpoint.trim().is_empty() {
                        return Err(AppError::validation_error(
                            "SSE/HTTP 连接类型需要端点 URL 不能为空",
                        ));
                    }
                } else {
                    return Err(AppError::validation_error("SSE/HTTP 连接类型需要端点 URL"));
                }
            }
        }

        // 检查是否需要重新连接刷新元数据
        let connection_params_changed = request.command.is_some()
            || request.args.is_some()
            || request.working_dir.is_some()
            || request.env.is_some()
            || request.connection_type.is_some()
            || request.endpoint.is_some()
            || request.headers.is_some()
            || request.timeout_ms.is_some();

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

        // 如果连接参数发生变化且服务器是启用状态，需要重新连接
        if connection_params_changed && existing.enabled {
            should_refresh = true;
        }

        existing.updated_at = Self::current_timestamp();

        if should_refresh {
            // 如果需要刷新且连接失败，返回错误，不保存更新
            self.update_server_status(&mut existing)
                .await
                .map_err(|e| AppError::internal_error(&e.to_string()))?;
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
            // toggle 时即使连接失败也保存错误状态
            let _ = self.update_server_status(&mut server).await;
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
        // refresh 时即使连接失败也保存错误状态
        let _ = self.update_server_status(&mut server).await;
        server.updated_at = Self::current_timestamp();
        self.repository.update_server(&server).await?;
        Ok(server)
    }

    /// Delete a server definition
    pub async fn delete_server(&self, server_id: String) -> Result<(), AppError> {
        self.repository.delete_server(&server_id).await
    }

    /// Update tool enabled status
    pub async fn update_tool_enabled(
        &self,
        request: UpdateToolEnabledRequest,
    ) -> Result<McpServer, AppError> {
        let mut server = self.get_server(&request.server_id).await?;

        // Update enabled_tools list
        if request.enabled {
            // Add tool if not already in list
            if !server.enabled_tools.contains(&request.tool_name) {
                server.enabled_tools.push(request.tool_name.clone());
            }
        } else {
            // Remove tool from list
            server
                .enabled_tools
                .retain(|name| name != &request.tool_name);
        }

        server.updated_at = Self::current_timestamp();
        self.repository.update_server(&server).await?;
        Ok(server)
    }

    /// Count chats using a specific MCP server
    pub async fn count_chats_using_server(&self, server_id: &str) -> Result<i32, AppError> {
        self.chat_repository
            .count_chats_using_mcp_server(server_id)
            .await
    }

    /// Remove MCP server references from all chats
    pub async fn remove_mcp_server_from_chats(&self, server_id: &str) -> Result<i32, AppError> {
        self.chat_repository
            .remove_mcp_server_from_chats(server_id)
            .await
    }

    /// Update server status and metadata based on connection type
    ///
    /// Returns Ok if connection succeeds, Err if connection fails
    async fn update_server_status(&self, server: &mut McpServer) -> Result<(), McpClientError> {
        // All connection types now work the same way - try to connect and fetch metadata
        match self.fetch_server_metadata(server).await {
            Ok((tools, prompts, resources)) => {
                // If enabled_tools is empty (new server), enable all tools by default
                if server.enabled_tools.is_empty() && !tools.is_empty() {
                    server.enabled_tools = tools.iter().map(|t| t.name.clone()).collect();
                }

                server.tools = tools;
                server.prompts = prompts;
                server.resources = resources;
                server.status = McpServerStatus::Ready;
                server.last_error = None;
                server.last_sync_at = Some(Self::current_timestamp());
                Ok(())
            }
            Err(error) => {
                tracing::error!(
                    "Failed to fetch MCP metadata for {}: {}",
                    server.name,
                    error
                );
                // 连接失败时清空工具、提示、资源数据
                server.tools = Vec::new();
                server.prompts = Vec::new();
                server.resources = Vec::new();
                server.enabled_tools = Vec::new();
                server.status = McpServerStatus::Error;

                // 创建详细的错误信息
                use handbox_mcp::McpErrorDetail;

                // 直接从 McpClientError 提取信息
                server.last_error = Some(McpErrorDetail {
                    error_type: error.error_type(),
                    message: error.to_string(),
                    timestamp: Self::current_timestamp(),
                });

                Err(error)
            }
        }
    }

    fn validate_create_request(request: &CreateMcpServerRequest) -> Result<(), AppError> {
        if request.name.trim().is_empty() {
            return Err(AppError::validation_error("MCP 服务器名称不能为空"));
        }

        // 根据连接类型验证必填字段
        match request.connection_type {
            crate::storage::types::McpConnectionType::Stdio => {
                if request.command.trim().is_empty() {
                    return Err(AppError::validation_error("stdio 连接类型需要命令不能为空"));
                }
            }
            crate::storage::types::McpConnectionType::Sse
            | crate::storage::types::McpConnectionType::Http => {
                if let Some(ref endpoint) = request.endpoint {
                    if endpoint.trim().is_empty() {
                        return Err(AppError::validation_error(
                            "SSE/HTTP 连接类型需要端点 URL 不能为空",
                        ));
                    }
                } else {
                    return Err(AppError::validation_error("SSE/HTTP 连接类型需要端点 URL"));
                }
            }
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

    async fn fetch_server_metadata(
        &self,
        server: &McpServer,
    ) -> Result<(Vec<McpTool>, Vec<McpPrompt>, Vec<McpResource>), McpClientError> {
        let client = Self::connect_client(server).await?;

        // Fetch all metadata concurrently
        let tools_result = client.list_tools().await;
        let prompts_result = client.list_prompts().await;
        let resources_result = client.list_resources().await;

        // Gracefully shutdown the client
        if let Err(e) = client.shutdown().await {
            tracing::warn!(
                "Failed to gracefully shutdown MCP client for {}: {}",
                server.name,
                e
            );
        }

        // Convert results
        let tools = tools_result?;

        // Handle prompts - ignore "Method not found" error (-32601)
        let prompts = match prompts_result {
            Ok(p) => Self::convert_prompts(p),
            Err(McpClientError::Service(rmcp::service::ServiceError::McpError(ref error)))
                if error.code.0 == -32601 =>
            {
                tracing::debug!(
                    "list_prompts not supported by server {}, ignoring",
                    server.name
                );
                Vec::new()
            }
            Err(e) => return Err(e.into()),
        };

        // Handle resources - ignore "Method not found" error (-32601)
        let resources = match resources_result {
            Ok(r) => Self::convert_resources(r),
            Err(McpClientError::Service(rmcp::service::ServiceError::McpError(ref error)))
                if error.code.0 == -32601 =>
            {
                tracing::debug!(
                    "list_resources not supported by server {}, ignoring",
                    server.name
                );
                Vec::new()
            }
            Err(e) => return Err(e.into()),
        };

        Ok((tools, prompts, resources))
    }

    fn convert_prompts(prompts: Vec<rmcp::model::Prompt>) -> Vec<McpPrompt> {
        prompts
            .into_iter()
            .map(|p| McpPrompt {
                name: p.name.to_string(),
                description: p.description.map(|d| d.to_string()),
                arguments: p
                    .arguments
                    .unwrap_or_default()
                    .into_iter()
                    .map(|a| McpPromptArgument {
                        name: a.name.to_string(),
                        description: a.description.map(|d| d.to_string()),
                        required: a.required,
                    })
                    .collect(),
            })
            .collect()
    }

    fn convert_resources(resources: Vec<rmcp::model::Resource>) -> Vec<McpResource> {
        resources
            .into_iter()
            .map(|r| {
                // Convert annotations to HashMap if present
                let annotations = if let Some(ref annot) = r.annotations {
                    // Try to serialize and deserialize to convert to HashMap
                    serde_json::from_value(serde_json::to_value(annot).unwrap_or_default())
                        .unwrap_or_default()
                } else {
                    HashMap::new()
                };

                McpResource {
                    uri: r.uri.to_string(),
                    name: r.name.to_string(),
                    description: r.description.as_ref().map(|d| d.to_string()),
                    mime_type: r.mime_type.as_ref().map(|m| m.to_string()),
                    annotations,
                }
            })
            .collect()
    }

    async fn connect_client(server: &McpServer) -> Result<McpClient, McpClientError> {
        if !server.enabled {
            return Err(McpClientError::TransportCreation(format!(
                "MCP server '{}' is not enabled",
                server.name
            )));
        }

        Self::validate_server_configuration(server)?;
        let config = Self::build_connection_config(server)?;

        McpClient::connect(config).await
    }

    fn validate_server_configuration(server: &McpServer) -> Result<(), McpClientError> {
        match server.connection_type {
            crate::storage::types::McpConnectionType::Stdio => {
                validate_server_config(&server.command, &server.working_dir, &server.env).map_err(
                    |e| {
                        McpClientError::TransportCreation(format!(
                            "Invalid stdio configuration for MCP server '{}': {}",
                            server.name, e
                        ))
                    },
                )?;
            }
            crate::storage::types::McpConnectionType::Sse
            | crate::storage::types::McpConnectionType::Http => {
                let endpoint = server.endpoint.as_ref().ok_or_else(|| {
                    McpClientError::TransportCreation(format!(
                        "Endpoint is required for {}",
                        server.connection_type
                    ))
                })?;

                if endpoint.trim().is_empty() {
                    return Err(McpClientError::TransportCreation(format!(
                        "Endpoint is required for {} connection type",
                        server.connection_type.as_str()
                    )));
                }
            }
        }

        Ok(())
    }

    fn build_connection_config(server: &McpServer) -> Result<ConnectionConfig, McpClientError> {
        match server.connection_type {
            crate::storage::types::McpConnectionType::Stdio => {
                let mut config = ProcessConfig::new(&server.command)
                    .with_args(server.args.clone())
                    .with_env(server.env.clone());

                if let Some(ref working_dir) = server.working_dir {
                    config = config.with_working_dir(working_dir.clone());
                }

                Ok(ConnectionConfig::Process(config))
            }
            crate::storage::types::McpConnectionType::Sse => {
                let endpoint = server.endpoint.as_ref().ok_or_else(|| {
                    McpClientError::TransportCreation(
                        "Endpoint is required for SSE connection".to_string(),
                    )
                })?;

                let mut config =
                    SseConfig::new(endpoint.clone()).with_headers(server.headers.clone());

                if let Some(timeout_ms) = server.timeout_ms {
                    config = config.with_timeout(timeout_ms);
                }

                Ok(ConnectionConfig::Sse(config))
            }
            crate::storage::types::McpConnectionType::Http => {
                let endpoint = server.endpoint.as_ref().ok_or_else(|| {
                    McpClientError::TransportCreation(
                        "Endpoint is required for HTTP connection".to_string(),
                    )
                })?;

                let mut config = StreamableHttpConfig::new(endpoint.clone())
                    .with_headers(server.headers.clone());

                if let Some(timeout_ms) = server.timeout_ms {
                    config = config.with_timeout(timeout_ms);
                }

                Ok(ConnectionConfig::Http(config))
            }
        }
    }

    /// 执行工具调用（通过工具名称和参数）
    pub async fn execute_tool(&self, tool_name: &str, arguments: &str) -> Result<String, AppError> {
        // 获取活跃的 MCP 服务器
        let servers = self
            .list_servers()
            .await?
            .into_iter()
            .filter(|s| s.enabled)
            .collect::<Vec<_>>();

        // 在所有服务器中查找工具
        for server in &servers {
            if let Some(tool) = server.tools.iter().find(|t| t.name == tool_name) {
                let arguments = Self::parse_tool_arguments(arguments);

                match self.invoke_tool(&server, &tool.name, arguments).await {
                    Ok(result) => return Ok(Self::format_tool_result(&result)),
                    Err(error) => {
                        tracing::error!(
                            "[McpService::execute_tool] Tool {} failed: {}",
                            tool_name,
                            error
                        );
                        // McpClientError 会自动转换为 AppError
                        return Err(error.into());
                    }
                }
            }
        }

        // 如果没有找到工具
        tracing::warn!(
            "[McpService::execute_tool] Tool {} not found in any MCP server",
            tool_name
        );
        Err(AppError::not_found(&format!(
            "工具 {} 未在任何 MCP 服务器中找到",
            tool_name
        )))
    }

    /// 调用特定服务器上的工具
    async fn invoke_tool(
        &self,
        server: &McpServer,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<rmcp::model::CallToolResult, McpClientError> {
        let client = Self::connect_client(server).await?;

        let call_result = client.call_tool(tool_name, arguments).await;

        if let Err(e) = client.shutdown().await {
            tracing::warn!(
                "[McpService::invoke_tool] Failed to shutdown MCP client {}: {}",
                server.name,
                e
            );
        }

        call_result
    }

    /// 解析工具参数
    fn parse_tool_arguments(arguments: &str) -> Option<serde_json::Value> {
        if arguments.trim().is_empty() {
            return None;
        }

        match serde_json::from_str::<serde_json::Value>(arguments) {
            Ok(serde_json::Value::Object(map)) => Some(serde_json::Value::Object(map)),
            Ok(other) => {
                let mut wrapper = serde_json::Map::new();
                wrapper.insert("value".to_string(), other);
                Some(serde_json::Value::Object(wrapper))
            }
            Err(_) => {
                let mut wrapper = serde_json::Map::new();
                wrapper.insert(
                    "raw".to_string(),
                    serde_json::Value::String(arguments.trim().to_string()),
                );
                Some(serde_json::Value::Object(wrapper))
            }
        }
    }

    /// 格式化工具调用结果
    fn format_tool_result(result: &rmcp::model::CallToolResult) -> String {
        if let Some(structured) = &result.structured_content {
            return serde_json::to_string_pretty(structured)
                .unwrap_or_else(|_| structured.to_string());
        }

        let mut pieces = Vec::new();
        for content in &result.content {
            match &content.raw {
                rmcp::model::RawContent::Text(text) => pieces.push(text.text.clone()),
                _ => pieces.push(
                    serde_json::to_string(&content).unwrap_or_else(|_| format!("{:?}", content)),
                ),
            }
        }

        if pieces.is_empty() {
            "工具未返回任何内容".to_string()
        } else {
            pieces.join("\n")
        }
    }
}
