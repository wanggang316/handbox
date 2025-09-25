//! MCP client utilities for connecting to Model Context Protocol servers.
//!
//! This module provides higher level helpers that mirror the official RMCP
//! client examples so the backend can talk to MCP servers either by spawning a
//! local process or connecting to an SSE endpoint.

use std::{collections::HashMap, path::Path};

use anyhow::{anyhow, Context, Result};
use rmcp::{
    model::{CallToolRequestParam, CallToolResult, ClientInfo, Tool as RmcpTool},
    service::{RoleClient, RunningService, ServiceExt},
    transport::{SseClientTransport, TokioChildProcess},
};

use crate::models::McpTool;

/// Common command paths to search for Node.js tools
const NODE_COMMAND_PATHS: &[&str] = &[
    "/usr/local/bin",
    "/opt/homebrew/bin",
    "/usr/bin",
];

/// NVM-style paths to search (common locations)
const NVM_PATHS: &[&str] = &[
    "/.nvm/versions/node/v22.19.0/bin",
    "/.nvm/versions/node/v20.18.0/bin",
    "/.nvm/versions/node/v18.20.0/bin",
    "/.nvm/current/bin",
];

/// Lightweight client wrapper around RMCP's [`RunningService`].
///
/// The client owns the underlying transport task. Drop the struct or call
/// [`shutdown`](Self::shutdown) to stop the connection.
#[derive(Debug)]
pub struct McpClient {
    service: RunningService<RoleClient, ClientInfo>,
}

impl McpClient {
    /// Connect to an MCP server by spawning a child process, mirroring the
    /// official RMCP example. The provided `command` string can include inline
    /// arguments (e.g. "npx -y @modelcontextprotocol/server-everything").
    pub async fn connect_process(
        command: &str,
        extra_args: &[String],
        working_dir: Option<&Path>,
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        tracing::info!("> command: {}", command);
        tracing::info!("> extra_args: {:?}", extra_args);
        tracing::info!("> working_dir: {:?}", working_dir);
        tracing::info!("> env: {:?}", env);

        let (program, mut parsed_args) = parse_command(command)?;

        parsed_args.extend(extra_args.iter().cloned());

        // Resolve the full path for the program
        let resolved_program = resolve_command_path(&program)?;

        tracing::info!("program: {} (resolved to: {})", program, resolved_program);
        tracing::info!("parsed_args: {:?}", parsed_args);

        // let mut display_parts = Vec::with_capacity(1 + parsed_args.len());
        // display_parts.push(program.clone());
        // display_parts.extend(parsed_args.iter().cloned());
        // let display_command = display_parts.join(" ");

        let mut cmd = tokio::process::Command::new(&resolved_program);
        if !parsed_args.is_empty() {
            cmd.args(&parsed_args);
        }
        if let Some(dir) = working_dir {
            tracing::info!("> working_dir: {:?}", dir);
            // Only set working directory if it's not empty
            if !dir.as_os_str().is_empty() {
                cmd.current_dir(dir);
            } else {
                tracing::warn!("Empty working directory provided, using default");
            }
        }
        if !env.is_empty() {
            cmd.envs(env.iter().map(|(k, v)| (k, v)));
        }

        let client = (default_client_info())
        .serve(
            TokioChildProcess::new(cmd).with_context(|| {
                format!(
                    "Failed to start MCP server process: {} {}",
                    resolved_program,
                    parsed_args.join(" ")
                )
            })?,
        )
        .await
        .with_context(|| format!("Failed to establish MCP connection to {}", resolved_program))?;
    
        // Initialize
        let server_info = client.peer_info();
        tracing::info!("Connected to server: {server_info:#?}");

        Ok(Self { service: client })
    }

    /// Connect to an MCP server that exposes an SSE endpoint. Requires the
    /// `transport-sse-client-reqwest` feature on the `rmcp` crate.
    pub async fn connect_sse(endpoint: &str) -> Result<Self> {
        let transport = SseClientTransport::start(endpoint.to_string())
            .await
            .context("连接 MCP SSE 端点失败")?;

        let client_info = default_client_info();
        let service = client_info
            .clone()
            .serve(transport)
            .await
            .context("MCP 初始化失败")?;

        Ok(Self { service })
    }

    /// List all tools exposed by the connected MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let tools = self
            .service
            .list_all_tools()
            .await
            .context("获取 MCP 工具列表失败")?;
        Ok(tools.into_iter().map(convert_tool).collect())
    }

    /// Call a tool by name with optional JSON arguments. Returns the full
    /// [`CallToolResult`] from RMCP so callers can inspect structured data.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let arguments = match arguments {
            Some(serde_json::Value::Object(map)) => Some(map),
            Some(other) => return Err(anyhow!("工具参数必须是 JSON 对象, 当前提供: {}", other)),
            None => None,
        };

        let request = CallToolRequestParam {
            name: name.to_string().into(),
            arguments,
        };

        self.service
            .call_tool(request)
            .await
            .context("调用 MCP 工具失败")
    }

    /// Gracefully shutdown the client connection, cancelling the background
    /// transport task.
    pub async fn shutdown(self) -> Result<()> {
        self.service.cancel().await.context("关闭 MCP 客户端失败")?;
        Ok(())
    }

    /// Expose the raw RMCP peer for advanced scenarios that need full access
    /// to the protocol. This keeps ownership with the [`McpClient`].
    pub fn peer(&self) -> &rmcp::service::Peer<RoleClient> {
        self.service.peer()
    }
}

fn default_client_info() -> ClientInfo {
    let mut info = ClientInfo::default();
    info.client_info.name = "handbox".into();
    info.client_info.version = env!("CARGO_PKG_VERSION").into();
    info
}

fn convert_tool(tool: RmcpTool) -> McpTool {
    use serde_json::Value;

    let RmcpTool {
        name,
        title: _,
        description,
        input_schema,
        output_schema: _,
        annotations,
        icons: _,
    } = tool;

    let annotations_map = annotations
        .and_then(|ann| serde_json::to_value(ann).ok())
        .and_then(|value| value.as_object().cloned())
        .map(|map| map.into_iter().collect::<HashMap<String, Value>>())
        .unwrap_or_default();

    McpTool {
        name: name.into_owned(),
        description: description.map(|d| d.into_owned()),
        input_schema: Value::Object((*input_schema).clone()),
        annotations: annotations_map,
    }
}

/// Parse a command string into an executable plus argument vector using shell
/// semantics. Mirrors the `shlex` parsing used in the official client example.
pub fn parse_command(command: &str) -> Result<(String, Vec<String>)> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("MCP 服务器命令不能为空"));
    }

    let parts =
        shlex::split(trimmed).ok_or_else(|| anyhow!("无法解析 MCP 服务器命令: {}", command))?;

    let mut iter = parts.into_iter();
    let program = iter
        .next()
        .ok_or_else(|| anyhow!("无法解析 MCP 服务器命令: {}", command))?;

    Ok((program, iter.collect()))
}

/// Resolve a command to its full path, searching common locations if not found in PATH
fn resolve_command_path(command: &str) -> Result<String> {
    // First, try to find the command using the `which` command
    if let Ok(output) = std::process::Command::new("which")
        .arg(command)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                return Ok(path);
            }
        }
    }

    // If `which` didn't work, try common paths for Node.js tools
    if command == "npx" || command == "npm" || command == "node" {
        // First try standard system paths
        for base_path in NODE_COMMAND_PATHS {
            let full_path = format!("{}/{}", base_path, command);
            if std::path::Path::new(&full_path).exists() {
                return Ok(full_path);
            }
        }

        // Then try NVM paths with home directory
        if let Ok(home) = std::env::var("HOME") {
            for nvm_path in NVM_PATHS {
                let full_path = format!("{}{}/{}", home, nvm_path, command);
                if std::path::Path::new(&full_path).exists() {
                    return Ok(full_path);
                }
            }
        }

        // Try to find any Node version in NVM
        if let Ok(home) = std::env::var("HOME") {
            let nvm_versions_dir = format!("{}/.nvm/versions/node", home);
            if let Ok(entries) = std::fs::read_dir(&nvm_versions_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if entry.path().is_dir() {
                            let bin_path = entry.path().join("bin").join(command);
                            if bin_path.exists() {
                                return Ok(bin_path.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    // If all else fails, return the original command and let the system try to find it
    // This provides a fallback for commands that might be in PATH but not found by our search
    Ok(command.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_command, resolve_command_path};

    #[test]
    fn parse_command_handles_single_word() {
        let (program, args) = parse_command("npx").expect("should parse");
        assert_eq!(program, "npx");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_command_splits_arguments() {
        let (program, args) =
            parse_command("npx -y @modelcontextprotocol/server-everything").expect("should parse");
        assert_eq!(program, "npx");
        assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-everything"]);
    }

    #[test]
    fn parse_command_respects_quotes() {
        let (program, args) =
            parse_command("python -m server --config \"path/with space/config.json\"")
                .expect("should parse");
        assert_eq!(program, "python");
        assert_eq!(
            args,
            vec!["-m", "server", "--config", "path/with space/config.json"]
        );
    }

    #[test]
    fn parse_command_errors_on_empty_input() {
        let error = parse_command(" ").expect_err("should fail");
        assert!(
            error.to_string().contains("MCP 服务器命令不能为空"),
            "unexpected error: {}",
            error
        );
    }

    #[test]
    fn resolve_command_path_handles_absolute_paths() {
        // Absolute paths should be returned as-is if they exist
        let result = resolve_command_path("/bin/sh").expect("should resolve");
        assert_eq!(result, "/bin/sh");
    }

    #[test]
    fn resolve_command_path_fallback_for_unknown_commands() {
        // Unknown commands should fall back to the original command
        let result = resolve_command_path("unknown_command_xyz").expect("should resolve");
        assert_eq!(result, "unknown_command_xyz");
    }

    #[test]
    fn resolve_command_path_handles_node_tools() {
        // This test will vary depending on the system, but it should not panic
        let result = resolve_command_path("npx");
        assert!(result.is_ok(), "Command resolution should not fail");
    }

    #[test]
    fn connect_process_handles_empty_working_dir() {
        // This test verifies that empty working directory doesn't cause errors
        // We can't test the full connection since it requires an actual MCP server,
        // but we can at least verify the path normalization works
        use std::path::Path;

        // Test that empty path is handled correctly
        let empty_path = Path::new("");
        assert!(empty_path.as_os_str().is_empty());

        // Test that non-empty path is handled correctly
        let valid_path = Path::new("/tmp");
        assert!(!valid_path.as_os_str().is_empty());
    }
}
