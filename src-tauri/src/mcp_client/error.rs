//! Error types for MCP client operations.

use thiserror::Error;

/// Errors that can occur during MCP client operations.
#[derive(Error, Debug)]
pub enum McpClientError {
    /// Connection establishment failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Transport layer error
    #[error("Transport error: {0}")]
    TransportError(String),

    /// MCP protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// Request timeout
    #[error("Request timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// Authentication/authorization error
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Invalid configuration
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Tool not found
    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    /// Invalid tool arguments
    #[error("Invalid tool arguments: {0}")]
    InvalidToolArguments(String),

    /// Resource not found
    #[error("Resource '{0}' not found")]
    ResourceNotFound(String),

    /// Prompt not found
    #[error("Prompt '{0}' not found")]
    PromptNotFound(String),

    /// Service is disconnected
    #[error("Service is disconnected")]
    Disconnected,

    /// Cancelled operation
    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    /// Client initialization error from rmcp
    #[error("Client initialization error: {0}")]
    ClientInitializeError(String),

    /// Service error from rmcp
    #[error("Service error: {0}")]
    ServiceError(#[from] rmcp::service::ServiceError),

    /// Other errors
    #[error("Unknown error: {0}")]
    Other(String),
}

impl McpClientError {
    /// Create a connection failed error
    pub fn connection_failed(msg: impl Into<String>) -> Self {
        Self::ConnectionFailed(msg.into())
    }

    /// Create a transport error
    pub fn transport_error(msg: impl Into<String>) -> Self {
        Self::TransportError(msg.into())
    }

    /// Create a protocol error
    pub fn protocol_error(msg: impl Into<String>) -> Self {
        Self::ProtocolError(msg.into())
    }

    /// Create a configuration error
    pub fn config_error(msg: impl Into<String>) -> Self {
        Self::ConfigurationError(msg.into())
    }

    /// Create an invalid tool arguments error
    pub fn invalid_tool_args(msg: impl Into<String>) -> Self {
        Self::InvalidToolArguments(msg.into())
    }

    /// Check if this is a connection-related error
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed(_) | Self::TransportError(_) | Self::Disconnected
        )
    }

    /// Check if this is a timeout error
    pub fn is_timeout(&self) -> bool {
        matches!(self, Self::Timeout(_))
    }

    /// Check if this is an authentication error
    pub fn is_auth_error(&self) -> bool {
        matches!(self, Self::AuthenticationError(_))
    }
}

/// Result type for MCP client operations.
pub type McpClientResult<T> = Result<T, McpClientError>;

/// Convert anyhow::Error to McpClientError
impl From<anyhow::Error> for McpClientError {
    fn from(err: anyhow::Error) -> Self {
        // Convert error to string representation
        let error_str = err.to_string();

        // Try to classify based on error message patterns
        let error_lower = error_str.to_lowercase();

        if error_lower.contains("connection")
            || error_lower.contains("transport")
            || error_lower.contains("refused")
        {
            return Self::ConnectionFailed(error_str);
        }

        if error_lower.contains("timeout") || error_lower.contains("timed out") {
            return Self::Timeout(std::time::Duration::from_secs(30));
        }

        if error_lower.contains("auth") || error_lower.contains("unauthorized") {
            return Self::AuthenticationError(error_str);
        }

        if error_lower.contains("protocol") || error_lower.contains("parse") {
            return Self::ProtocolError(error_str);
        }

        // Otherwise, convert to Other
        Self::Other(error_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_constructors() {
        let err = McpClientError::connection_failed("test");
        assert!(err.is_connection_error());

        let err = McpClientError::config_error("invalid config");
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_error_display() {
        let err = McpClientError::ToolNotFound("echo".to_string());
        assert_eq!(err.to_string(), "Tool 'echo' not found");
    }
}
