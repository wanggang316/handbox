//! High-level MCP client interface.
//!
//! This module provides a clean, modern client interface for connecting to MCP servers
//! using either process-based or SSE transports.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rmcp::{
    model::{CallToolRequestParam, CallToolResult, ClientInfo},
    service::{RoleClient, RunningService, ServiceExt},
};
use serde_json::Value;

use crate::models::McpTool;

use super::{
    process::ProcessTransport,
    sse::SseTransport,
    types::{ClientStats, ConnectionConfig, ConnectionStatus, ProcessConfig, SseConfig},
    utils::convert_tool,
};

/// High-level MCP client that can connect via different transports
pub struct McpClient {
    service: RunningService<RoleClient, ClientInfo>,
    stats: Arc<Mutex<ClientStats>>,
    status: Arc<Mutex<ConnectionStatus>>,
}

impl McpClient {
    /// Connect to an MCP server using the provided configuration
    pub async fn connect(config: ConnectionConfig) -> Result<Self> {
        match config {
            ConnectionConfig::Process(process_config) => {
                Self::connect_process(process_config).await
            }
            ConnectionConfig::Sse(sse_config) => {
                Self::connect_sse(sse_config).await
            }
        }
    }

    /// Connect to an MCP server using process transport
    pub async fn connect_process(config: ProcessConfig) -> Result<Self> {
        let mut stats = ClientStats::new();

        let transport = ProcessTransport::new(config).await.map_err(|e| {
            tracing::error!("Failed to create process transport: {}", e);
            e
        })?;

        let client_info = create_client_info();
        let service = client_info
            .serve(transport)
            .await
            .context("Failed to establish MCP connection")?;

        // Log connection info
        let server_info = service.peer();
        tracing::info!("Connected to MCP server: {:#?}", server_info);

        // Update stats and status
        stats.set_connected(chrono::Utc::now().timestamp_millis());

        Ok(Self {
            service,
            stats: Arc::new(Mutex::new(stats)),
            status: Arc::new(Mutex::new(ConnectionStatus::Connected)),
        })
    }

    /// Connect to an MCP server using SSE transport (not yet implemented)
    pub async fn connect_sse(config: SseConfig) -> Result<Self> {
        tracing::info!("Attempting SSE connection to: {}", config.endpoint);

        // For now, we validate the endpoint and return a descriptive error
        SseTransport::validate_endpoint(&config.endpoint)
            .context("Invalid SSE endpoint")?;

        // Try to create the transport to provide better error messages
        SseTransport::new(config).await.map_err(|e| {
            tracing::error!("SSE transport failed: {}", e);
            anyhow::anyhow!(
                "SSE/HTTP transport is not yet implemented. Please use 'stdio' connection type for process-based MCP servers. Error: {}",
                e
            )
        })?;

        // This line will never be reached due to the error above, but is needed for type checking
        unreachable!()
    }

    /// List all tools exposed by the connected MCP server
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let tools = self
            .service
            .list_all_tools()
            .await
            .context("Failed to list MCP tools")?;

        Ok(tools.into_iter().map(convert_tool).collect())
    }

    /// Call a tool by name with optional JSON arguments
    pub async fn call_tool(&self, name: &str, arguments: Option<Value>) -> Result<CallToolResult> {
        let arguments = match arguments {
            Some(Value::Object(map)) => Some(map),
            Some(other) => {
                self.record_error();
                return Err(anyhow::anyhow!(
                    "Tool arguments must be a JSON object, got: {}",
                    other
                ));
            }
            None => None,
        };

        let request = CallToolRequestParam {
            name: name.to_string().into(),
            arguments,
        };

        let result = self
            .service
            .call_tool(request)
            .await
            .with_context(|| format!("Failed to call MCP tool '{}'", name));

        if result.is_ok() {
            self.record_tool_call();
        } else {
            self.record_error();
        }

        result
    }

    /// List all resources exposed by the server
    pub async fn list_resources(&self) -> Result<Vec<rmcp::model::Resource>> {
        self.service
            .list_all_resources()
            .await
            .context("Failed to list MCP resources")
    }

    /// Read a specific resource
    pub async fn read_resource(&self, uri: &str) -> Result<rmcp::model::ReadResourceResult> {
        let request = rmcp::model::ReadResourceRequestParam {
            uri: uri.to_string().into(),
        };

        let result = self
            .service
            .read_resource(request)
            .await
            .with_context(|| format!("Failed to read resource '{}'", uri));

        if result.is_ok() {
            self.record_resource_read();
        } else {
            self.record_error();
        }

        result
    }

    /// List all prompts exposed by the server
    pub async fn list_prompts(&self) -> Result<Vec<rmcp::model::Prompt>> {
        self.service
            .list_all_prompts()
            .await
            .context("Failed to list MCP prompts")
    }

    /// Get a specific prompt
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, Value>>,
    ) -> Result<rmcp::model::GetPromptResult> {
        let request = rmcp::model::GetPromptRequestParam {
            name: name.to_string().into(),
            arguments,
        };

        let result = self
            .service
            .get_prompt(request)
            .await
            .with_context(|| format!("Failed to get prompt '{}'", name));

        if result.is_ok() {
            self.record_prompt_request();
        } else {
            self.record_error();
        }

        result
    }

    /// Get current connection status
    pub fn status(&self) -> ConnectionStatus {
        self.status.lock().unwrap().clone()
    }

    /// Get client statistics
    pub fn stats(&self) -> ClientStats {
        self.stats.lock().unwrap().clone()
    }

    /// Get server information
    pub fn server_info(&self) -> &rmcp::service::Peer<rmcp::service::RoleClient> {
        self.service.peer()
    }

    /// Gracefully shutdown the client connection
    pub async fn shutdown(self) -> Result<()> {
        *self.status.lock().unwrap() = ConnectionStatus::Disconnected;

        self.service
            .cancel()
            .await
            .context("Failed to shutdown MCP client")?;
        Ok(())
    }

    /// Get raw service for advanced operations
    pub fn service(&self) -> &RunningService<RoleClient, ClientInfo> {
        &self.service
    }

    // Private helper methods for stats tracking
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

/// Create default client info
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
