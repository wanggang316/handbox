//! MCP client utilities for connecting to Model Context Protocol servers.
//!
//! This module provides a modern, clean client implementation following the official
//! Rust SDK patterns. It supports process-based connections with robust error handling
//! and connection management.
//!
//! # Examples
//!
//! ## Basic usage with process transport
//! ```no_run
//! use handbox_lib::mcp_client::{McpClient, ProcessConfig, ConnectionConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ProcessConfig::new("npx")
//!         .with_args(vec!["-y".to_string(), "@modelcontextprotocol/server-everything".to_string()]);
//!
//!     let client = McpClient::connect_process(config).await?;
//!     let tools = client.list_tools().await?;
//!
//!     println!("Available tools: {}", tools.len());
//!     client.shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Using the factory pattern
//! ```no_run
//! use handbox_lib::mcp_client::{McpClientFactory, McpClientManager};
//! use handbox_lib::models::McpServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let server = McpServer {
//!         // ... server configuration
//!         # id: "test".to_string(),
//!         # name: "test".to_string(),
//!         # display_name: None,
//!         # description: None,
//!         # command: "npx".to_string(),
//!         # args: vec!["-y".to_string(), "@modelcontextprotocol/server-everything".to_string()],
//!         # working_dir: None,
//!         # env: std::collections::HashMap::new(),
//!         # enabled: true,
//!         # status: handbox_lib::models::McpServerStatus::Ready,
//!         # tools: vec![],
//!         # last_sync_at: None,
//!         # last_error: None,
//!         # created_at: 0,
//!         # updated_at: 0,
//!     };
//!
//!     let client = McpClientFactory::create_client(&server).await?;
//!
//!     let mut manager = McpClientManager::new();
//!     manager.add_client(server.id.clone(), client);
//!
//!     // Use the clients...
//!
//!     manager.shutdown_all().await;
//!     Ok(())
//! }
//! ```

mod client;
mod factory;
mod process;
mod sse;
mod types;
mod utils;

// Re-export the main interfaces
pub use client::McpClient;
pub use factory::{McpClientFactory, McpClientManager};
pub use types::{ClientStats, ConnectionConfig, ConnectionStatus, ProcessConfig, SseConfig};

// Re-export utilities that might be useful for advanced users
pub use utils::{create_server_display_name, resolve_command_path, validate_server_config};
