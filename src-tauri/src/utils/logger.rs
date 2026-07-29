use crate::models::AppError;

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

pub fn log_error(context: &str, error: &dyn std::error::Error) {
    tracing::error!("{}: {}", context, error);
}

pub fn log_warning(context: &str, message: &str) {
    tracing::warn!("{}: {}", context, message);
}

pub fn log_info(context: &str, message: &str) {
    tracing::info!("{}: {}", context, message);
}
