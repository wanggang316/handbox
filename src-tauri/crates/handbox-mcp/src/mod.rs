//! MCP client utilities for connecting to Model Context Protocol servers.
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
mod client;
mod error;
mod process;
mod sse;
mod streamable_http;
pub mod types;
mod utils;

pub use client::McpClient;
pub use error::{McpClientError, McpClientResult};
pub use types::{
    ClientStats, ConnectionConfig, ConnectionStatus, McpErrorDetail, McpPrompt, McpPromptArgument,
    McpResource, McpTool, ProcessConfig, SseConfig, StreamableHttpConfig,
};

pub use utils::{create_server_display_name, resolve_command_path, validate_server_config};
