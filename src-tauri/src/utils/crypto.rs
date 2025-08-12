// 加密工具函数

use crate::models::AppError;

/// 生成 UUID
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// API Key 脱敏显示
pub fn mask_api_key(api_key: &str, prefix_length: usize) -> String {
    if api_key.len() <= prefix_length {
        "*".repeat(api_key.len())
    } else {
        let prefix = &api_key[..prefix_length];
        let masked_length = api_key.len() - prefix_length;
        format!("{}{}...", prefix, "*".repeat(masked_length.min(8)))
    }
}

/// 验证 API Key 格式
pub fn validate_api_key(api_key: &str) -> Result<(), AppError> {
    if api_key.is_empty() {
        return Err(AppError::validation_error("API Key cannot be empty"));
    }

    if api_key.len() < 8 {
        return Err(AppError::validation_error("API Key is too short"));
    }

    if api_key.contains(' ') {
        return Err(AppError::validation_error("API Key cannot contain spaces"));
    }

    Ok(())
}

/// 从 Keychain 获取 API Key
pub async fn get_api_key_from_keychain(_service: &str, _account: &str) -> Result<String, AppError> {
    // TODO: 实现 Keychain 集成
    // 使用 keyring crate 或 platform-specific APIs
    Err(AppError::internal_error(
        "Keychain integration not implemented yet",
    ))
}

/// 保存 API Key 到 Keychain
pub async fn save_api_key_to_keychain(
    _service: &str,
    _account: &str,
    api_key: &str,
) -> Result<(), AppError> {
    validate_api_key(api_key)?;

    // TODO: 实现 Keychain 集成
    Err(AppError::internal_error(
        "Keychain integration not implemented yet",
    ))
}

/// 从 Keychain 删除 API Key
pub async fn delete_api_key_from_keychain(_service: &str, _account: &str) -> Result<(), AppError> {
    // TODO: 实现 Keychain 集成
    Err(AppError::internal_error(
        "Keychain integration not implemented yet",
    ))
}
