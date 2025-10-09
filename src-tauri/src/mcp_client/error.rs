//! Error types for MCP client operations.

use std::fmt;

/// Errors that can occur during MCP client operations.
///
/// This error type wraps rmcp library errors and adds client-specific error variants.
#[derive(Debug)]
pub enum McpClientError {
    /// Transport creation failed (e.g., process spawn, HTTP connection)
    TransportCreation(String),

    /// Client initialization failed
    ClientInitialize(rmcp::service::ClientInitializeError),

    /// Service operation failed
    Service(rmcp::service::ServiceError),

    /// Runtime error (task join error)
    Runtime(tokio::task::JoinError),

    /// Invalid tool arguments provided
    InvalidToolArguments(String),

    /// Client shutdown failed
    Shutdown(String),
}

impl std::error::Error for McpClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ClientInitialize(e) => Some(e),
            Self::Service(e) => Some(e),
            Self::Runtime(e) => Some(e),
            _ => None,
        }
    }
}

// Implement From traits for automatic conversion
impl From<rmcp::service::ClientInitializeError> for McpClientError {
    fn from(e: rmcp::service::ClientInitializeError) -> Self {
        Self::ClientInitialize(e)
    }
}

impl From<rmcp::service::ServiceError> for McpClientError {
    fn from(e: rmcp::service::ServiceError) -> Self {
        Self::Service(e)
    }
}

impl From<tokio::task::JoinError> for McpClientError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Runtime(e)
    }
}

impl fmt::Display for McpClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransportCreation(msg) => write!(f, "Transport creation failed: {}", msg),
            Self::ClientInitialize(e) => {
                match e {
                    rmcp::service::ClientInitializeError::TransportError { error, context } => {
                        // Replace DynamicTransportError with clean underlying error
                        write!(f, "Send message error {}, when {}", error.error, context)
                    }
                    // For all other errors, use their Display implementation
                    _ => write!(f, "{}", e),
                }
            }
            Self::Service(e) => {
                match e {
                    rmcp::service::ServiceError::McpError(mcp_err) => {
                        // MCP errors use their own message
                        write!(f, "{}", mcp_err.message)
                    }
                    rmcp::service::ServiceError::TransportSend(dyn_err) => {
                        // Replace DynamicTransportError with clean underlying error
                        write!(f, "Transport send error: {}", dyn_err.error)
                    }
                    // For all other errors, use their Display implementation
                    _ => write!(f, "{}", e),
                }
            }
            Self::Runtime(e) => write!(f, "{}", e),
            Self::InvalidToolArguments(msg) => write!(f, "Invalid tool arguments: {}", msg),
            Self::Shutdown(msg) => write!(f, "Shutdown failed: {}", msg),
        }
    }
}

impl McpClientError {
    /// Get error type classification for database storage
    pub fn error_type(&self) -> String {
        match self {
            Self::TransportCreation(_) => "Transport Creation".to_string(),
            Self::ClientInitialize(e) => match e {
                rmcp::service::ClientInitializeError::ExpectedInitResponse(_) => {
                    "Client Init: Expected Init Response".to_string()
                }
                rmcp::service::ClientInitializeError::ExpectedInitResult(_) => {
                    "Client Init: Expected Init Result".to_string()
                }
                rmcp::service::ClientInitializeError::ConflictInitResponseId(_, _) => {
                    "Client Init: Conflict Response ID".to_string()
                }
                rmcp::service::ClientInitializeError::ConnectionClosed(_) => {
                    "Client Init: Connection Closed".to_string()
                }
                rmcp::service::ClientInitializeError::TransportError { .. } => {
                    "Client Init: Transport Error".to_string()
                }
                rmcp::service::ClientInitializeError::Cancelled => {
                    "Client Init: Cancelled".to_string()
                }
            },
            Self::Service(e) => match e {
                rmcp::service::ServiceError::McpError(mcp_err) => {
                    format!("MCP Error: {}", mcp_err.code.0)
                }
                rmcp::service::ServiceError::TransportSend(_) => {
                    "Service: Transport Send".to_string()
                }
                rmcp::service::ServiceError::TransportClosed => {
                    "Service: Transport Closed".to_string()
                }
                rmcp::service::ServiceError::UnexpectedResponse => {
                    "Service: Unexpected Response".to_string()
                }
                rmcp::service::ServiceError::Cancelled { .. } => "Service: Cancelled".to_string(),
                rmcp::service::ServiceError::Timeout { .. } => "Service: Timeout".to_string(),
                _ => "Service: Unknown".to_string(),
            },
            Self::Runtime(_) => "Runtime Error".to_string(),
            Self::InvalidToolArguments(_) => "Invalid Tool Arguments".to_string(),
            Self::Shutdown(_) => "Shutdown Failed".to_string(),
        }
    }
}

/// Result type for MCP client operations.
pub type McpClientResult<T> = Result<T, McpClientError>;
