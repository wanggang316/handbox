// 错误类型定义

use serde::{Deserialize, Serialize};

/// 应用错误类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub hint: Option<String>,
}

impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            hint: None,
        }
    }

    pub fn with_hint(code: &str, message: &str, hint: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            hint: Some(hint.to_string()),
        }
    }

    // 预定义的错误类型
    pub fn validation_error(message: &str) -> Self {
        Self::with_hint("VALIDATION_ERROR", message, "请检查输入参数")
    }

    pub fn auth_error(message: &str) -> Self {
        Self::with_hint("AUTH_ERROR", message, "请检查API密钥配置")
    }

    pub fn network_error(message: &str) -> Self {
        Self::with_hint("NETWORK_ERROR", message, "请检查网络连接")
    }

    pub fn rate_limit_error() -> Self {
        Self::with_hint("RATE_LIMIT", "请求过于频繁", "请稍后重试或降低请求频率")
    }

    pub fn internal_error(message: &str) -> Self {
        Self::with_hint("INTERNAL_ERROR", message, "应用内部错误，请重新启动应用")
    }

    pub fn not_found(message: &str) -> Self {
        Self::with_hint("NOT_FOUND", message, "请求的资源未找到")
    }

    // 供应商相关错误
    pub fn provider_name_exists() -> Self {
        Self::with_hint("PROVIDER_NAME_EXISTS", "供应商名称已存在", "请使用其他名称")
    }

    pub fn provider_api_key_invalid() -> Self {
        Self::with_hint(
            "PROVIDER_API_KEY_INVALID",
            "API Key 无效",
            "请检查 API Key 是否正确",
        )
    }

    pub fn provider_api_endpoint_invalid() -> Self {
        Self::with_hint(
            "PROVIDER_API_ENDPOINT_INVALID",
            "API 端点配置错误或服务不可用",
            "请检查 Base URL 和供应商类型配置",
        )
    }

    pub fn provider_api_permission_denied() -> Self {
        Self::with_hint(
            "PROVIDER_API_PERMISSION_DENIED",
            "API Key 权限不足",
            "请检查 API Key 的权限设置",
        )
    }

    pub fn provider_models_fetch_failed() -> Self {
        Self::with_hint(
            "PROVIDER_MODELS_FETCH_FAILED",
            "获取供应商模型失败",
            "请检查网络连接和配置",
        )
    }

    /// 将 LLM API 错误转换为更友好的供应商错误
    pub fn from_llm_fetch_error(error: handbox_llm::LlmClientError) -> Self {
        let app_error: AppError = error.into();

        // 检查是否是配置相关错误（API Key、端点等）
        if app_error.code == "INTERNAL_ERROR"
            && (app_error.message.contains("Incorrect API key")
                || app_error.message.contains("invalid_api_key")
                || app_error.message.contains("API key not valid")
                || app_error.message.contains("400 Bad Request")
                || app_error.message.contains("401 Unauthorized")
                || app_error.message.contains("404 Not Found")
                || app_error.message.contains("403 Forbidden"))
        {
            // 对于配置错误，返回特定的错误类型
            if app_error.message.contains("404 Not Found") {
                Self::provider_api_endpoint_invalid()
            } else if app_error.message.contains("403 Forbidden") {
                Self::provider_api_permission_denied()
            } else {
                Self::provider_api_key_invalid()
            }
        } else {
            // 对于其他错误（如网络问题），返回原始错误
            app_error
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

// sqlx 错误转换
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::internal_error(&format!("Database error: {}", error))
    }
}

// MCP client 错误转换
impl From<handbox_mcp::McpClientError> for AppError {
    fn from(error: handbox_mcp::McpClientError) -> Self {
        Self::new("MCP_ERROR", &error.to_string())
    }
}

// LLM client 错误转换
impl From<handbox_llm::LlmClientError> for AppError {
    fn from(error: handbox_llm::LlmClientError) -> Self {
        use handbox_llm::LlmClientError;

        match error {
            LlmClientError::Validation(message) => Self::validation_error(&message),
            LlmClientError::Configuration(message)
            | LlmClientError::ClientInitialization(message)
            | LlmClientError::Unexpected(message) => Self::internal_error(&message),
            LlmClientError::Transport(message) => Self::network_error(&message),
            LlmClientError::Api(message) => Self::internal_error(&message),
        }
    }
}

/// API 响应包装类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "success")]
pub enum ApiResponse<T> {
    #[serde(rename = "true")]
    Success { data: T },
    #[serde(rename = "false")]
    Error { error: AppError },
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self::Success { data }
    }

    pub fn error(error: AppError) -> Self {
        Self::Error { error }
    }
}

impl<T> From<Result<T, AppError>> for ApiResponse<T> {
    fn from(result: Result<T, AppError>) -> Self {
        match result {
            Ok(data) => Self::success(data),
            Err(error) => Self::error(error),
        }
    }
}
