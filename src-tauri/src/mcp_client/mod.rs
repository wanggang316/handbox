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
        let (program, mut parsed_args) = parse_command(command)?;
        parsed_args.extend(extra_args.iter().cloned());

        let mut display_parts = Vec::with_capacity(1 + parsed_args.len());
        display_parts.push(program.clone());
        display_parts.extend(parsed_args.iter().cloned());
        let display_command = display_parts.join(" ");

        let mut cmd = tokio::process::Command::new(&program);
        if !parsed_args.is_empty() {
            cmd.args(&parsed_args);
        }
        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        if !env.is_empty() {
            cmd.envs(env.iter().map(|(k, v)| (k, v)));
        }

        let transport = TokioChildProcess::new(cmd)
            .with_context(|| format!("无法启动 MCP 服务器命令: {}", display_command))?;

        let client_info = default_client_info();
        let service = client_info
            .clone()
            .serve(transport)
            .await
            .context("MCP 初始化失败")?;

        Ok(Self { service })
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

#[cfg(test)]
mod tests {
    use super::parse_command;

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
}
