//! Streamable HTTP MCP transport implementation.
//!
//! This transport uses the rmcp streamable HTTP client, configured with reqwest.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

use super::sse::SseTransport;
use super::types::StreamableHttpConfig;

/// Streamable HTTP transport for MCP servers
#[allow(clippy::module_name_repetitions)]
pub struct StreamableHttpTransport;

impl StreamableHttpTransport {
    /// Create a new streamable HTTP transport for the given configuration
    pub fn connect(
        config: &StreamableHttpConfig,
    ) -> Result<StreamableHttpClientTransport<reqwest::Client>> {
        tracing::info!("Creating MCP streamable HTTP transport");
        tracing::info!("> endpoint: {}", config.endpoint);

        SseTransport::validate_endpoint(&config.endpoint)?;

        let client = Self::build_http_client(config)
            .context("Failed to build HTTP client for streamable transport")?;

        let mut transport_config =
            StreamableHttpClientTransportConfig::with_uri(config.endpoint.clone());
        transport_config.allow_stateless = config.allow_stateless;
        if let Some(auth_value) = Self::authorization_header(&config.headers) {
            transport_config.auth_header = Some(auth_value.clone());
        }

        Ok(StreamableHttpClientTransport::with_client(
            client,
            transport_config,
        ))
    }

    fn build_http_client(config: &StreamableHttpConfig) -> Result<reqwest::Client> {
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
            .context("Failed to build reqwest client for streamable HTTP transport")
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

    fn authorization_header(headers: &HashMap<String, String>) -> Option<String> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("authorization"))
            .map(|(_, value)| value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_header_extracts_value_case_insensitively() {
        let header = StreamableHttpTransport::authorization_header(&HashMap::from([(
            "AUTHORIZATION".to_string(),
            "Bearer token".to_string(),
        )]));
        assert_eq!(header, Some("Bearer token".to_string()));
    }

    #[test]
    fn authorization_header_returns_none_when_missing() {
        let header = StreamableHttpTransport::authorization_header(&HashMap::new());
        assert!(header.is_none());
    }
}
