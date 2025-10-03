//! SSE (Server-Sent Events) MCP transport implementation.
//!
//! Provides an rmcp-compatible SSE client transport built on top of reqwest.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rmcp::transport::sse_client::SseClientConfig;
use rmcp::transport::SseClientTransport;

use super::types::SseConfig;

/// SSE transport for MCP servers powered by reqwest
#[allow(clippy::module_name_repetitions)]
pub struct SseTransport;

impl SseTransport {
    /// Create a new SSE transport connecting to the given endpoint
    pub async fn connect(config: &SseConfig) -> Result<SseClientTransport<reqwest::Client>> {
        tracing::info!("Creating MCP SSE transport");
        tracing::info!("> endpoint: {}", config.endpoint);

        Self::validate_endpoint(&config.endpoint)?;

        let client = Self::build_http_client(config)
            .context("Failed to build HTTP client for SSE transport")?;

        let mut transport_config = SseClientConfig {
            sse_endpoint: config.endpoint.clone().into(),
            ..Default::default()
        };

        if let Some(message_endpoint) = &config.message_endpoint {
            transport_config.use_message_endpoint = Some(message_endpoint.clone());
        }

        SseClientTransport::start_with_client(client, transport_config)
            .await
            .context("Failed to start SSE client transport")
    }

    fn build_http_client(config: &SseConfig) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder();

        if let Some(timeout_ms) = config.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }

        if !config.headers.is_empty() {
            let headers = Self::build_header_map(&config.headers)?;
            builder = builder.default_headers(headers);
        }

        builder
            .build()
            .context("Failed to build reqwest client for SSE transport")
    }

    fn build_header_map(headers: &HashMap<String, String>) -> Result<HeaderMap> {
        let mut header_map = HeaderMap::new();
        for (key, value) in headers {
            let name = HeaderName::from_bytes(key.as_bytes())
                .with_context(|| format!("Invalid header name: {}", key))?;
            let header_value = HeaderValue::from_str(value)
                .with_context(|| format!("Invalid header value for header '{}': {}", key, value))?;
            header_map.insert(name, header_value);
        }
        Ok(header_map)
    }

    /// Validate that the endpoint URL is well-formed
    pub fn validate_endpoint(endpoint: &str) -> Result<()> {
        if endpoint.trim().is_empty() {
            return Err(anyhow::anyhow!("SSE endpoint cannot be empty"));
        }

        if endpoint.starts_with("ftp://") || endpoint.starts_with("file://") {
            return Err(anyhow::anyhow!(
                "Unsupported protocol for SSE endpoint: {}",
                endpoint
            ));
        }

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
