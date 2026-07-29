//! High-level MCP client interface.
//!
//! Connects to MCP servers over stdio child processes, SSE endpoints, or
//! streamable HTTP transports.

use std::sync::{Arc, Mutex};

use rmcp::{
    model::{CallToolRequestParam, CallToolResult, ClientInfo},
    service::{RoleClient, RunningService, ServiceExt},
};
use serde_json::Value;

use crate::types::McpTool;

use super::{
    error::{McpClientError, McpClientResult},
    process::ProcessTransport,
    sse::SseTransport,
    streamable_http::StreamableHttpTransport,
    types::{
        ClientStats, ConnectionConfig, ConnectionStatus, ProcessConfig, SseConfig,
        StreamableHttpConfig,
    },
    utils::convert_tool,
};

pub struct McpClient {
    service: RunningService<RoleClient, ClientInfo>,
    stats: Arc<Mutex<ClientStats>>,
    status: Arc<Mutex<ConnectionStatus>>,
}

impl McpClient {
    pub async fn connect(config: ConnectionConfig) -> McpClientResult<Self> {
        match config {
            ConnectionConfig::Process(process_config) => {
                Self::connect_process(process_config).await
            }
            ConnectionConfig::Sse(sse_config) => Self::connect_sse(sse_config).await,
            ConnectionConfig::Http(http_config) => Self::connect_http(http_config).await,
        }
    }

    pub async fn connect_process(config: ProcessConfig) -> McpClientResult<Self> {
        let stats = ClientStats::new();

        let transport = ProcessTransport::new(config).await.map_err(|e| {
            tracing::error!("Failed to create process transport: {}", e);
            McpClientError::TransportCreation(e.to_string())
        })?;

        let client_info = create_client_info();
        let service = client_info.serve(transport).await?;

        let server_info = service.peer();
        tracing::info!("Connected to MCP server: {:#?}", server_info);

        Ok(Self::from_service(service, stats))
    }

    pub async fn connect_sse(config: SseConfig) -> McpClientResult<Self> {
        tracing::info!("Attempting SSE connection to: {}", config.endpoint);

        let stats = ClientStats::new();

        let transport = SseTransport::connect(&config).await.map_err(|e| {
            tracing::error!("Failed to create SSE transport: {}", e);
            McpClientError::TransportCreation(e.to_string())
        })?;

        let client_info = create_client_info();
        let service = client_info.serve(transport).await?;

        let server_info = service.peer();
        tracing::info!("Connected to MCP server via SSE: {:#?}", server_info);

        Ok(Self::from_service(service, stats))
    }

    pub async fn connect_http(config: StreamableHttpConfig) -> McpClientResult<Self> {
        tracing::info!(
            "Attempting streamable HTTP connection to: {}",
            config.endpoint
        );

        let stats = ClientStats::new();

        let transport = StreamableHttpTransport::connect(&config).map_err(|e| {
            tracing::error!("Failed to create streamable HTTP transport: {}", e);
            McpClientError::TransportCreation(e.to_string())
        })?;

        let client_info = create_client_info();
        let service = client_info.serve(transport).await?;

        let server_info = service.peer();
        tracing::info!("Connected to MCP server via HTTP: {:#?}", server_info);

        Ok(Self::from_service(service, stats))
    }

    pub async fn list_tools(&self) -> McpClientResult<Vec<McpTool>> {
        let tools = self.service.list_all_tools().await.map_err(|e| {
            tracing::error!("Failed to list MCP tools: {}", e);
            McpClientError::from(e)
        })?;

        Ok(tools.into_iter().map(convert_tool).collect())
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> McpClientResult<CallToolResult> {
        let arguments = match arguments {
            Some(Value::Object(map)) => Some(map),
            Some(other) => {
                self.record_error();
                return Err(McpClientError::InvalidToolArguments(format!(
                    "Tool arguments must be a JSON object, got: {}",
                    other
                )));
            }
            None => None,
        };

        let request = CallToolRequestParam {
            name: name.to_string().into(),
            arguments,
        };

        let result = self.service.call_tool(request).await.map_err(|e| {
            tracing::error!("Failed to call MCP tool '{}': {}", name, e);
            self.record_error();
            McpClientError::from(e)
        });

        if result.is_ok() {
            self.record_tool_call();
        }

        result
    }

    pub async fn list_resources(&self) -> McpClientResult<Vec<rmcp::model::Resource>> {
        self.service.list_all_resources().await.map_err(|e| {
            tracing::error!("Failed to list MCP resources: {}", e);
            e.into()
        })
    }

    pub async fn read_resource(
        &self,
        uri: &str,
    ) -> McpClientResult<rmcp::model::ReadResourceResult> {
        let request = rmcp::model::ReadResourceRequestParam {
            uri: uri.to_string().into(),
        };

        let result = self.service.read_resource(request).await.map_err(|e| {
            tracing::error!("Failed to read resource '{}': {}", uri, e);
            self.record_error();
            McpClientError::from(e)
        });

        if result.is_ok() {
            self.record_resource_read();
        }

        result
    }

    pub async fn list_prompts(&self) -> McpClientResult<Vec<rmcp::model::Prompt>> {
        self.service.list_all_prompts().await.map_err(|e| {
            tracing::error!("Failed to list MCP prompts: {}", e);
            e.into()
        })
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, Value>>,
    ) -> McpClientResult<rmcp::model::GetPromptResult> {
        let request = rmcp::model::GetPromptRequestParam {
            name: name.to_string().into(),
            arguments,
        };

        let result = self.service.get_prompt(request).await.map_err(|e| {
            tracing::error!("Failed to get prompt '{}': {}", name, e);
            self.record_error();
            McpClientError::from(e)
        });

        if result.is_ok() {
            self.record_prompt_request();
        }

        result
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn stats(&self) -> ClientStats {
        self.stats.lock().unwrap().clone()
    }

    pub fn server_info(&self) -> &rmcp::service::Peer<rmcp::service::RoleClient> {
        self.service.peer()
    }

    pub async fn shutdown(self) -> McpClientResult<()> {
        *self.status.lock().unwrap() = ConnectionStatus::Disconnected;

        self.service.cancel().await.map_err(|e| {
            tracing::error!("Failed to shutdown MCP client: {}", e);
            McpClientError::Shutdown(e.to_string())
        })?;
        Ok(())
    }

    pub fn service(&self) -> &RunningService<RoleClient, ClientInfo> {
        &self.service
    }

    fn from_service(
        service: RunningService<RoleClient, ClientInfo>,
        mut stats: ClientStats,
    ) -> Self {
        if stats.connected_since.is_none() {
            stats.set_connected(chrono::Utc::now().timestamp_millis());
        }

        Self {
            service,
            stats: Arc::new(Mutex::new(stats)),
            status: Arc::new(Mutex::new(ConnectionStatus::Connected)),
        }
    }

    fn record_tool_call(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.tool_called();
        }
    }

    fn record_resource_read(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.resource_read();
        }
    }

    fn record_prompt_request(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.prompt_requested();
        }
    }

    fn record_error(&self) {
        if let Ok(mut stats) = self.stats.lock() {
            stats.error_occurred();
        }
    }
}

fn create_client_info() -> ClientInfo {
    let mut info = ClientInfo::default();
    info.client_info.name = "handbox".into();
    info.client_info.version = env!("CARGO_PKG_VERSION").into();
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_info_has_correct_values() {
        let info = create_client_info();
        assert_eq!(info.client_info.name, "handbox");
        assert_eq!(info.client_info.version, env!("CARGO_PKG_VERSION"));
    }
}
