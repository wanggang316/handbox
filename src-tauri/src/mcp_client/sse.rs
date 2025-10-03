//! SSE (Server-Sent Events) MCP transport implementation.
//!
//! This module handles connecting to MCP servers that expose SSE endpoints
//! for real-time communication.

use anyhow::{Context, Result};
use rmcp::transport::TokioChildProcess;
use std::process::Stdio;
use tokio::process::Command;

use super::types::SseConfig;

/// SSE transport for MCP servers (currently implemented using HTTP proxy)
pub struct SseTransport;

impl SseTransport {
    /// Create a new SSE transport connecting to the given endpoint
    /// For now, this creates a simple HTTP client to test connectivity
    pub async fn new(config: SseConfig) -> Result<TokioChildProcess> {
        tracing::info!("Creating MCP SSE transport (HTTP-based)");
        tracing::info!("> endpoint: {}", config.endpoint);
        tracing::info!("> headers: {:?}", config.headers);
        tracing::info!("> timeout_ms: {:?}", config.timeout_ms);

        // Validate endpoint
        Self::validate_endpoint(&config.endpoint)?;

        // For now, create a simple curl-based proxy to the SSE endpoint
        // This is a temporary solution until rmcp SSE transport issues are resolved
        let mut cmd = Command::new("curl");
        cmd.arg("-N") // No buffering
            .arg("-H")
            .arg("Accept: text/event-stream")
            .arg("-H")
            .arg("Cache-Control: no-cache");

        // Add custom headers
        for (key, value) in &config.headers {
            cmd.arg("-H").arg(format!("{}: {}", key, value));
        }

        // Add timeout if specified
        if let Some(timeout_ms) = config.timeout_ms {
            cmd.arg("--max-time").arg((timeout_ms / 1000).to_string());
        }

        cmd.arg(&config.endpoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        TokioChildProcess::new(cmd)
            .context("Failed to start SSE transport via curl")
    }

    /// Validate that the endpoint URL is well-formed
    pub fn validate_endpoint(endpoint: &str) -> Result<()> {
        if endpoint.trim().is_empty() {
            return Err(anyhow::anyhow!("SSE endpoint cannot be empty"));
        }

        // Basic URL validation - reject non-HTTP(S) protocols
        if endpoint.starts_with("ftp://") || endpoint.starts_with("file://") {
            return Err(anyhow::anyhow!(
                "Unsupported protocol for SSE endpoint: {}",
                endpoint
            ));
        }

        // Must have some form of URL structure
        if !endpoint.contains("://")
            && !endpoint.starts_with("localhost")
            && !endpoint.contains(':')
        {
            return Err(anyhow::anyhow!("Invalid SSE endpoint format: {}", endpoint));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_endpoint_accepts_valid_urls() {
        assert!(SseTransport::validate_endpoint("http://localhost:8000/sse").is_ok());
        assert!(SseTransport::validate_endpoint("https://example.com/mcp").is_ok());
        assert!(SseTransport::validate_endpoint("localhost:3000").is_ok());
    }

    #[test]
    fn validate_endpoint_rejects_empty_urls() {
        assert!(SseTransport::validate_endpoint("").is_err());
        assert!(SseTransport::validate_endpoint("   ").is_err());
    }

    #[test]
    fn validate_endpoint_rejects_malformed_urls() {
        assert!(SseTransport::validate_endpoint("not-a-url").is_err());
        assert!(SseTransport::validate_endpoint("ftp://example.com").is_err());
    }
}
