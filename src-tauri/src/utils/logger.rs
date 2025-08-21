// 日志工具函数

use crate::models::AppError;

/// 初始化日志系统
pub fn init_logger() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init()
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize logger: {e}")))?;

    Ok(())
}

/// 记录错误日志
pub fn log_error(context: &str, error: &dyn std::error::Error) {
    tracing::error!("{}: {}", context, error);
}

/// 记录警告日志
pub fn log_warning(context: &str, message: &str) {
    tracing::warn!("{}: {}", context, message);
}

/// 记录信息日志
pub fn log_info(context: &str, message: &str) {
    tracing::info!("{}: {}", context, message);
}
