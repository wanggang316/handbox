//! Types for MCP client operations.
//!
//! This module contains all the type definitions used by the MCP client,
//! providing a clean separation between data types and implementation logic.

use std::collections::HashMap;

/// Configuration for process-based MCP connections
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub command: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    pub env: HashMap<String, String>,
}

impl ProcessConfig {
    /// Create a new process configuration with the given command
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            working_dir: None,
            env: HashMap::new(),
        }
    }

    /// Add arguments to the command
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set environment variables
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }
}

/// Configuration for SSE-based MCP connections
#[derive(Debug, Clone)]
pub struct SseConfig {
    pub endpoint: String,
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub message_endpoint: Option<String>,
}

impl SseConfig {
    /// Create a new SSE configuration with the given endpoint
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            headers: HashMap::new(),
            timeout_ms: None,
            message_endpoint: None,
        }
    }

    /// Add HTTP headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Override the message endpoint advertised by the server
    pub fn with_message_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.message_endpoint = Some(endpoint.into());
        self
    }
}

/// Configuration for streamable HTTP MCP connections
#[derive(Debug, Clone)]
pub struct StreamableHttpConfig {
    pub endpoint: String,
    pub headers: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub allow_stateless: bool,
}

impl StreamableHttpConfig {
    /// Create a new HTTP configuration with the given endpoint
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            headers: HashMap::new(),
            timeout_ms: None,
            allow_stateless: true,
        }
    }

    /// Add HTTP headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = headers;
        self
    }

    /// Set request timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Control whether requests can be stateless
    pub fn with_allow_stateless(mut self, allow_stateless: bool) -> Self {
        self.allow_stateless = allow_stateless;
        self
    }
}

/// Connection type for MCP servers
#[derive(Debug, Clone)]
pub enum ConnectionConfig {
    /// Process-based connection
    Process(ProcessConfig),
    /// Server-Sent Events connection
    Sse(SseConfig),
    /// Streamable HTTP connection
    Http(StreamableHttpConfig),
}

impl ConnectionConfig {
    /// Create a process connection config
    pub fn process(command: impl Into<String>) -> Self {
        Self::Process(ProcessConfig::new(command))
    }

    /// Create an SSE connection config
    pub fn sse(endpoint: impl Into<String>) -> Self {
        Self::Sse(SseConfig::new(endpoint))
    }

    /// Create a streamable HTTP connection config
    pub fn http(endpoint: impl Into<String>) -> Self {
        Self::Http(StreamableHttpConfig::new(endpoint))
    }
}

/// Client connection status
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// Not connected
    Disconnected,
    /// Connecting to server
    Connecting,
    /// Successfully connected
    Connected,
    /// Connection error
    Error(String),
}

/// MCP client statistics
#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    pub tools_called: u64,
    pub resources_read: u64,
    pub prompts_requested: u64,
    pub errors: u64,
    pub connected_since: Option<i64>,
}

impl ClientStats {
    /// Create new stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark that a tool was called
    pub fn tool_called(&mut self) {
        self.tools_called += 1;
    }

    /// Mark that a resource was read
    pub fn resource_read(&mut self) {
        self.resources_read += 1;
    }

    /// Mark that a prompt was requested
    pub fn prompt_requested(&mut self) {
        self.prompts_requested += 1;
    }

    /// Mark an error occurred
    pub fn error_occurred(&mut self) {
        self.errors += 1;
    }

    /// Set connection time
    pub fn set_connected(&mut self, timestamp: i64) {
        self.connected_since = Some(timestamp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_config_builder_works() {
        let config = ProcessConfig::new("npx")
            .with_args(vec!["-y".to_string(), "@mcp/server".to_string()])
            .with_working_dir("/tmp")
            .with_env(HashMap::from([(
                "NODE_ENV".to_string(),
                "development".to_string(),
            )]));

        assert_eq!(config.command, "npx");
        assert_eq!(config.args, vec!["-y", "@mcp/server"]);
        assert_eq!(config.working_dir, Some("/tmp".to_string()));
        assert_eq!(config.env.get("NODE_ENV"), Some(&"development".to_string()));
    }

    #[test]
    fn sse_config_builder_works() {
        let config = SseConfig::new("http://localhost:8000/sse")
            .with_headers(HashMap::from([(
                "Authorization".to_string(),
                "Bearer token".to_string(),
            )]))
            .with_timeout(5000)
            .with_message_endpoint("/custom-message");

        assert_eq!(config.endpoint, "http://localhost:8000/sse");
        assert_eq!(
            config.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(config.timeout_ms, Some(5000));
        assert_eq!(config.message_endpoint, Some("/custom-message".to_string()));
    }

    #[test]
    fn streamable_http_config_builder_works() {
        let config = StreamableHttpConfig::new("http://localhost:8000/mcp")
            .with_headers(HashMap::from([(
                "Authorization".to_string(),
                "Bearer token".to_string(),
            )]))
            .with_timeout(3000)
            .with_allow_stateless(false);

        assert_eq!(config.endpoint, "http://localhost:8000/mcp");
        assert_eq!(
            config.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert_eq!(config.timeout_ms, Some(3000));
        assert!(!config.allow_stateless);
    }

    #[test]
    fn connection_config_variants_work() {
        let process = ConnectionConfig::process("npx");
        let sse = ConnectionConfig::sse("http://localhost:8000/sse");
        let http = ConnectionConfig::http("http://localhost:8000/mcp");

        match process {
            ConnectionConfig::Process(config) => assert_eq!(config.command, "npx"),
            _ => panic!("Expected process config"),
        }

        match sse {
            ConnectionConfig::Sse(config) => {
                assert_eq!(config.endpoint, "http://localhost:8000/sse")
            }
            _ => panic!("Expected SSE config"),
        }

        match http {
            ConnectionConfig::Http(config) => {
                assert_eq!(config.endpoint, "http://localhost:8000/mcp")
            }
            _ => panic!("Expected HTTP config"),
        }
    }

    #[test]
    fn client_stats_tracking_works() {
        let mut stats = ClientStats::new();

        stats.tool_called();
        stats.resource_read();
        stats.prompt_requested();
        stats.error_occurred();
        stats.set_connected(1234567890);

        assert_eq!(stats.tools_called, 1);
        assert_eq!(stats.resources_read, 1);
        assert_eq!(stats.prompts_requested, 1);
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.connected_since, Some(1234567890));
    }
}
