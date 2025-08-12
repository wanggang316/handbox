// 验证工具函数

use crate::models::AppError;

/// 验证 UUID 格式
pub fn validate_uuid(uuid: &str) -> Result<(), AppError> {
    uuid::Uuid::parse_str(uuid).map_err(|_| AppError::validation_error("Invalid UUID format"))?;
    Ok(())
}

/// 验证 URL 格式
pub fn validate_url(url: &str) -> Result<(), AppError> {
    if url.is_empty() {
        return Err(AppError::validation_error("URL cannot be empty"));
    }

    // 基本的 URL 格式检查
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::validation_error(
            "URL must start with http:// or https://",
        ));
    }

    Ok(())
}

/// 验证模型参数
pub fn validate_temperature(temperature: f32) -> Result<(), AppError> {
    if temperature < 0.0 || temperature > 2.0 {
        return Err(AppError::validation_error(
            "Temperature must be between 0.0 and 2.0",
        ));
    }
    Ok(())
}

/// 验证 Top-P 参数
pub fn validate_top_p(top_p: f32) -> Result<(), AppError> {
    if top_p < 0.0 || top_p > 1.0 {
        return Err(AppError::validation_error(
            "Top-P must be between 0.0 and 1.0",
        ));
    }
    Ok(())
}
