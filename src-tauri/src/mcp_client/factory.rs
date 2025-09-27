//! Client factory and connection management for MCP clients.
//!
//! This module provides high-level interfaces for creating and managing
//! MCP client connections with automatic configuration handling.

use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::models::McpServer;

use super::{
    client::McpClient,
    types::{ConnectionConfig, ProcessConfig},
    utils::validate_server_config,
};

/// Factory for creating MCP clients from server configurations
pub struct McpClientFactory;

impl McpClientFactory {
    /// Create a new MCP client from server configuration
    pub async fn create_client(server: &McpServer) -> Result<McpClient> {
        tracing::info!("Creating MCP client for server: {}", server.name);

        if !server.enabled {
            return Err(anyhow::anyhow!(
                "MCP server '{}' is not enabled",
                server.name
            ));
        }

        // Validate the configuration
        validate_server_config(&server.command, &server.working_dir, &server.env)
            .with_context(|| format!("Invalid configuration for MCP server '{}'", server.name))?;

        // Determine connection type and create config
        let config = Self::create_connection_config(server)?;

        // Create the client
        McpClient::connect(config)
            .await
            .with_context(|| format!("Failed to connect to MCP server '{}'", server.name))
    }

    /// Create multiple clients concurrently
    pub async fn create_clients(servers: &[McpServer]) -> HashMap<String, Result<McpClient>> {
        let futures = servers.iter().map(|server| {
            let server_id = server.id.clone();
            async move {
                let result = Self::create_client(server).await;
                (server_id, result)
            }
        });

        let results = futures::future::join_all(futures).await;
        results.into_iter().collect()
    }

    /// Create connection configuration from server definition
    fn create_connection_config(server: &McpServer) -> Result<ConnectionConfig> {
        if Self::is_sse_endpoint(&server.command) {
            // SSE endpoint configuration (currently disabled)
            Err(anyhow::anyhow!("SSE endpoints are temporarily disabled"))
        } else {
            // Process configuration
            let mut config = ProcessConfig::new(&server.command)
                .with_args(server.args.clone())
                .with_env(server.env.clone());

            if let Some(ref working_dir) = server.working_dir {
                config = config.with_working_dir(working_dir.clone());
            }

            Ok(ConnectionConfig::Process(config))
        }
    }

    /// Determine if a command string represents an SSE endpoint
    fn is_sse_endpoint(command: &str) -> bool {
        command.starts_with("http://")
            || command.starts_with("https://")
            || (command.contains("://") && command.contains("/sse"))
            || (command.starts_with("localhost") && command.contains(":"))
    }
}

/// Manager for handling multiple MCP client connections
pub struct McpClientManager {
    clients: HashMap<String, McpClient>,
}

impl McpClientManager {
    /// Create a new client manager
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Add a client to the manager
    pub fn add_client(&mut self, server_id: String, client: McpClient) {
        self.clients.insert(server_id, client);
    }

    /// Get a client by server ID
    pub fn get_client(&self, server_id: &str) -> Option<&McpClient> {
        self.clients.get(server_id)
    }

    /// Get a mutable reference to a client by server ID
    pub fn get_client_mut(&mut self, server_id: &str) -> Option<&mut McpClient> {
        self.clients.get_mut(server_id)
    }

    /// Remove and shutdown a client
    pub async fn remove_client(&mut self, server_id: &str) -> Result<()> {
        if let Some(client) = self.clients.remove(server_id) {
            client
                .shutdown()
                .await
                .with_context(|| format!("Failed to shutdown client for server '{}'", server_id))?;
        }
        Ok(())
    }

    /// Get all active client IDs
    pub fn active_clients(&self) -> Vec<&String> {
        self.clients.keys().collect()
    }

    /// Count of active clients
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Shutdown all clients
    pub async fn shutdown_all(self) -> Vec<Result<()>> {
        let futures = self
            .clients
            .into_iter()
            .map(|(server_id, client)| async move {
                client.shutdown().await.with_context(|| {
                    format!("Failed to shutdown client for server '{}'", server_id)
                })
            });

        futures::future::join_all(futures).await
    }

    /// Check if a server is connected
    pub fn is_connected(&self, server_id: &str) -> bool {
        self.clients.contains_key(server_id)
    }

    /// Get all client statistics
    pub fn get_all_stats(&self) -> HashMap<&String, crate::mcp_client::types::ClientStats> {
        self.clients
            .iter()
            .map(|(id, client)| (id, client.stats()))
            .collect()
    }

    /// Get all server info
    pub fn get_all_server_info(
        &self,
    ) -> HashMap<&String, &rmcp::service::Peer<rmcp::service::RoleClient>> {
        self.clients
            .iter()
            .map(|(id, client)| (id, client.server_info()))
            .collect()
    }

    /// Create and add multiple clients from server configurations
    pub async fn connect_servers(&mut self, servers: &[McpServer]) -> HashMap<String, Result<()>> {
        let client_results = McpClientFactory::create_clients(servers).await;
        let mut results = HashMap::new();

        for (server_id, client_result) in client_results {
            match client_result {
                Ok(client) => {
                    self.add_client(server_id.clone(), client);
                    results.insert(server_id, Ok(()));
                }
                Err(e) => {
                    results.insert(server_id, Err(e));
                }
            }
        }

        results
    }
}

impl Default for McpClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_sse_endpoint_identifies_http_urls() {
        assert!(McpClientFactory::is_sse_endpoint(
            "http://localhost:8000/sse"
        ));
        assert!(McpClientFactory::is_sse_endpoint("https://example.com/mcp"));
        assert!(McpClientFactory::is_sse_endpoint("localhost:3000"));
    }

    #[test]
    fn is_sse_endpoint_rejects_commands() {
        assert!(!McpClientFactory::is_sse_endpoint("npx @mcp/server"));
        assert!(!McpClientFactory::is_sse_endpoint("python -m mcp_server"));
        assert!(!McpClientFactory::is_sse_endpoint("node server.js"));
    }

    #[test]
    fn client_manager_basic_operations() {
        let manager = McpClientManager::new();

        assert_eq!(manager.client_count(), 0);
        assert!(!manager.is_connected("test"));
        assert!(manager.get_client("test").is_none());

        let active = manager.active_clients();
        assert!(active.is_empty());
    }

    #[test]
    fn client_manager_default() {
        let manager = McpClientManager::default();
        assert_eq!(manager.client_count(), 0);
    }
}
