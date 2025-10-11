use thiserror::Error;

/// Error type for LLM client operations independent from application layer.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LlmClientError {
    /// Invalid input or unsupported option.
    #[error("Validation error: {0}")]
    Validation(String),
    /// Configuration related failure.
    #[error("Configuration error: {0}")]
    Configuration(String),
    /// Failure while constructing underlying clients.
    #[error("Client initialization error: {0}")]
    ClientInitialization(String),
    /// Failure while performing HTTP or SDK requests.
    #[error("Transport error: {0}")]
    Transport(String),
    /// Upstream API returned an error response.
    #[error("API error: {0}")]
    Api(String),
    /// Any other unexpected failure.
    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

impl LlmClientError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration(message.into())
    }

    pub fn client_initialization(message: impl Into<String>) -> Self {
        Self::ClientInitialization(message.into())
    }

    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport(message.into())
    }

    pub fn api(message: impl Into<String>) -> Self {
        Self::Api(message.into())
    }

    pub fn unexpected(message: impl Into<String>) -> Self {
        Self::Unexpected(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::unexpected(message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::transport(message)
    }
}

impl From<reqwest::Error> for LlmClientError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_status() {
            Self::api(err.to_string())
        } else {
            Self::transport(err.to_string())
        }
    }
}

impl From<serde_json::Error> for LlmClientError {
    fn from(err: serde_json::Error) -> Self {
        Self::unexpected(err.to_string())
    }
}
