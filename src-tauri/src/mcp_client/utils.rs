//! Utility functions for MCP client operations.
//!
//! This module contains helper functions for command resolution, data conversion,
//! and other common operations used by MCP clients.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use rmcp::model::Tool as RmcpTool;
use serde_json::Value;

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

/// Resolve a command to its full path, searching common locations if not found in PATH
pub fn resolve_command_path(command: &str) -> Result<String> {
    tracing::debug!("Resolving command path for: {}", command);

    // First, try to find the command using the `which` command
    if let Ok(output) = std::process::Command::new("which")
        .arg(command)
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                tracing::debug!("Found command via 'which': {}", path);
                return Ok(path);
            }
        }
    }

    // If `which` didn't work, try common paths for Node.js tools
    if command == "npx" || command == "npm" || command == "node" {
        tracing::debug!("Searching for Node.js tool in common paths");

        // First try standard system paths
        for base_path in NODE_COMMAND_PATHS {
            let full_path = format!("{}/{}", base_path, command);
            if std::path::Path::new(&full_path).exists() {
                tracing::debug!("Found Node.js tool at: {}", full_path);
                return Ok(full_path);
            }
        }

        // Then try NVM paths with home directory
        if let Ok(home) = std::env::var("HOME") {
            for nvm_path in NVM_PATHS {
                let full_path = format!("{}{}/{}", home, nvm_path, command);
                if std::path::Path::new(&full_path).exists() {
                    tracing::debug!("Found Node.js tool via NVM: {}", full_path);
                    return Ok(full_path);
                }
            }

            // Try to find any Node version in NVM
            let nvm_versions_dir = format!("{}/.nvm/versions/node", home);
            if let Ok(entries) = std::fs::read_dir(&nvm_versions_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if entry.path().is_dir() {
                            let bin_path = entry.path().join("bin").join(command);
                            if bin_path.exists() {
                                let path_str = bin_path.to_string_lossy().to_string();
                                tracing::debug!("Found Node.js tool in NVM version: {}", path_str);
                                return Ok(path_str);
                            }
                        }
                    }
                }
            }
        }
    }

    // If all else fails, return the original command and let the system try to find it
    // This provides a fallback for commands that might be in PATH but not found by our search
    tracing::debug!("Using fallback for command: {}", command);
    Ok(command.to_string())
}

/// Convert an RMCP tool to our internal McpTool representation
pub fn convert_tool(tool: RmcpTool) -> McpTool {
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

/// Validate MCP server configuration parameters
pub fn validate_server_config(
    command: &str,
    working_dir: &Option<String>,
    env: &HashMap<String, String>,
) -> Result<()> {
    // Validate command
    if command.trim().is_empty() {
        return Err(anyhow!("MCP server command cannot be empty"));
    }

    // Validate working directory if provided
    if let Some(dir) = working_dir {
        if !dir.trim().is_empty() {
            let path = std::path::Path::new(dir);
            if !path.exists() {
                tracing::warn!("Working directory does not exist: {}", dir);
                // Don't fail here, just warn - some servers might create the directory
            }
        }
    }

    // Validate environment variables
    for (key, _value) in env {
        if key.trim().is_empty() {
            return Err(anyhow!("Environment variable key cannot be empty"));
        }
    }

    Ok(())
}

/// Create a display name for MCP server configuration
pub fn create_server_display_name(
    command: &str,
    args: &[String],
    name: Option<&str>,
) -> String {
    if let Some(name) = name {
        if !name.trim().is_empty() {
            return name.to_string();
        }
    }

    // Extract meaningful parts from command
    let command_part = if command.contains('/') {
        command.split('/').last().unwrap_or(command)
    } else {
        command
    };

    if args.is_empty() {
        command_part.to_string()
    } else {
        // Include relevant arguments in display name
        let relevant_args: Vec<&str> = args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .take(2)
            .map(|s| s.as_str())
            .collect();

        if relevant_args.is_empty() {
            command_part.to_string()
        } else {
            format!("{} {}", command_part, relevant_args.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_command_path_handles_absolute_paths() {
        // This test assumes /bin/sh exists on Unix systems
        if cfg!(unix) {
            let result = resolve_command_path("/bin/sh").expect("should resolve");
            assert_eq!(result, "/bin/sh");
        }
    }

    #[test]
    fn resolve_command_path_fallback_for_unknown() {
        let result = resolve_command_path("unknown_command_xyz").expect("should resolve");
        assert_eq!(result, "unknown_command_xyz");
    }

    #[test]
    fn validate_server_config_accepts_valid_config() {
        let env = HashMap::new();
        assert!(validate_server_config("npx", &None, &env).is_ok());
        assert!(validate_server_config("node", &Some("/tmp".to_string()), &env).is_ok());
    }

    #[test]
    fn validate_server_config_rejects_empty_command() {
        let env = HashMap::new();
        assert!(validate_server_config("", &None, &env).is_err());
        assert!(validate_server_config("   ", &None, &env).is_err());
    }

    #[test]
    fn create_server_display_name_uses_provided_name() {
        let result = create_server_display_name("npx", &[], Some("My Server"));
        assert_eq!(result, "My Server");
    }

    #[test]
    fn create_server_display_name_extracts_from_command() {
        let result = create_server_display_name("/usr/bin/node", &[], None);
        assert_eq!(result, "node");

        let result = create_server_display_name("npx", &["-y".to_string(), "@mcp/server".to_string()], None);
        assert_eq!(result, "npx @mcp/server");
    }
}