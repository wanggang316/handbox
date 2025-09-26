//! Process-based MCP transport implementation.
//!
//! This module handles spawning MCP servers as child processes and establishing
//! communication via stdio.

use std::path::Path;

use anyhow::{Context, Result};
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;

use super::{types::ProcessConfig, utils::resolve_command_path};

/// Process transport for MCP servers
pub struct ProcessTransport;

impl ProcessTransport {
    /// Create a new process transport with the given configuration
    pub async fn new(config: ProcessConfig) -> Result<TokioChildProcess> {
        tracing::info!("Creating MCP process transport");
        tracing::info!("> command: {}", config.command);
        tracing::info!("> args: {:?}", config.args);
        tracing::info!("> working_dir: {:?}", config.working_dir);
        tracing::info!("> env: {:?}", config.env);

        // Parse and resolve the command
        let (program, parsed_args) = parse_command_with_args(&config.command, &config.args)?;
        let resolved_program = resolve_command_path(&program)?;

        tracing::info!("program: {} (resolved to: {})", program, resolved_program);
        tracing::info!("parsed_args: {:?}", parsed_args);

        // Build the command
        let mut cmd = Command::new(&resolved_program);

        if !parsed_args.is_empty() {
            cmd.args(&parsed_args);
        }

        // Set working directory if specified and not empty
        if let Some(ref working_dir) = config.working_dir {
            let working_dir: &String = working_dir;
            if !working_dir.trim().is_empty() {
                let path = Path::new(working_dir);
                if !path.as_os_str().is_empty() {
                    cmd.current_dir(path);
                    tracing::info!("Set working directory to: {}", working_dir);
                } else {
                    tracing::warn!("Empty working directory provided, using default");
                }
            }
        }

        // Set environment variables
        if !config.env.is_empty() {
            cmd.envs(&config.env);
            tracing::info!("Set {} environment variables", config.env.len());
        }

        // Create the transport
        TokioChildProcess::new(cmd).with_context(|| {
            format!(
                "Failed to start MCP server process: {} {}",
                resolved_program,
                parsed_args.join(" ")
            )
        })
    }
}

/// Parse command string and combine with additional arguments
fn parse_command_with_args(command: &str, extra_args: &[String]) -> Result<(String, Vec<String>)> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("MCP server command cannot be empty"));
    }

    // Parse the command string using shell-like syntax
    let parts = shlex::split(trimmed)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse MCP server command: {}", command))?;

    let mut iter = parts.into_iter();
    let program = iter
        .next()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse MCP server command: {}", command))?;

    let mut args: Vec<String> = iter.collect();
    args.extend(extra_args.iter().cloned());

    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_with_args_handles_simple_cases() {
        let (program, args) = parse_command_with_args("npx", &["-y".to_string()])
            .expect("should parse");
        assert_eq!(program, "npx");
        assert_eq!(args, vec!["-y"]);
    }

    #[test]
    fn parse_command_with_args_handles_complex_commands() {
        let (program, args) = parse_command_with_args(
            "npx -y @modelcontextprotocol/server-everything",
            &["--debug".to_string()]
        ).expect("should parse");
        assert_eq!(program, "npx");
        assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-everything", "--debug"]);
    }

    #[test]
    fn parse_command_with_args_handles_quotes() {
        let (program, args) = parse_command_with_args(
            r#"python -m server --config "path/with space/config.json""#,
            &[]
        ).expect("should parse");
        assert_eq!(program, "python");
        assert_eq!(args, vec!["-m", "server", "--config", "path/with space/config.json"]);
    }

    #[test]
    fn parse_command_with_args_errors_on_empty() {
        let result = parse_command_with_args("", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }
}