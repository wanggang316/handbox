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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_uuid() {
        let uuid = generate_uuid();
        assert!(!uuid.is_empty());
        assert_eq!(uuid.len(), 36); // UUID v4 standard length
    }

    #[test]
    fn test_mask_api_key() {
        // "sk-1234567890" has length 13, prefix 3 leaves 10 chars, min(10,8) = 8 stars
        assert_eq!(mask_api_key("sk-1234567890", 3), "sk-********...");
        assert_eq!(mask_api_key("short", 3), "sho**...");
        assert_eq!(mask_api_key("ab", 5), "**");
        assert_eq!(mask_api_key("", 3), "");
    }

    #[test]
    fn test_validate_api_key() {
        // Valid API key
        assert!(validate_api_key("sk-1234567890").is_ok());

        // Empty API key
        assert!(validate_api_key("").is_err());

        // Too short
        assert!(validate_api_key("short").is_err());

        // Contains spaces
        assert!(validate_api_key("sk-123 456").is_err());
    }

    // #[test]
    // fn test_keychain_service_and_account() {
    //     // 钥匙串功能测试暂时禁用
    // }

    // #[tokio::test]
    // async fn test_keychain_operations() {
    //     // 钥匙串功能测试暂时禁用
    // }
}
