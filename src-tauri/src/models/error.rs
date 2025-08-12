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
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

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
