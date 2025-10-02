//! SSE (Server-Sent Events) MCP transport implementation.
//!
//! This module handles connecting to MCP servers that expose SSE endpoints
//! for real-time communication.

use anyhow::Result;

use super::types::SseConfig;

/// SSE transport for MCP servers
pub struct SseTransport;

impl SseTransport {
    /// Create a new SSE transport connecting to the given endpoint
    /// Currently returns a placeholder implementation
    pub async fn new(config: SseConfig) -> Result<()> {
        tracing::info!("Creating MCP SSE transport");
        tracing::info!("> endpoint: {}", config.endpoint);
        tracing::info!("> headers: {:?}", config.headers);
        tracing::info!("> timeout_ms: {:?}", config.timeout_ms);

        // Validate endpoint
        Self::validate_endpoint(&config.endpoint)?;

        // TODO: Implement actual SSE transport once rmcp supports it
        // For now, return an error indicating it's not implemented
        Err(anyhow::anyhow!(
            "SSE transport is not yet implemented - please use stdio transport for now"
        ))
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
