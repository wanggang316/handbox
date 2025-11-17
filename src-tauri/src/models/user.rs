use serde::{Deserialize, Serialize};

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// 用户唯一标识
    pub id: String,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
    /// 头像 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// 是否为 Pro 用户
    pub is_pro: bool,
    /// 创建时间
    pub created_at: String,
    /// 更新时间
    pub updated_at: String,
}

/// 认证响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    /// 用户信息
    pub user: User,
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 令牌过期时间（秒）
    pub expires_in: u64,
}

/// Google 登录请求
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleLoginRequest {
    /// Google OAuth 授权码
    pub code: String,
    /// 重定向 URI
    pub redirect_uri: String,
}

/// 刷新令牌请求
#[derive(Debug, Clone, Deserialize)]
pub struct RefreshTokenRequest {
    /// 刷新令牌
    pub refresh_token: String,
}

/// 更新用户资料请求
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateUserProfileRequest {
    /// 用户名（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// 头像 URL（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_serialization() {
        let user = User {
            id: "user123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            avatar: Some("https://example.com/avatar.jpg".to_string()),
            is_pro: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&user).unwrap();
        assert!(json.contains("user123"));
        assert!(json.contains("testuser"));
        assert!(json.contains("test@example.com"));
    }

    #[test]
    fn test_user_without_avatar() {
        let user = User {
            id: "user123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            avatar: None,
            is_pro: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&user).unwrap();
        // avatar 字段为 None 时应该被跳过
        assert!(!json.contains("\"avatar\""));
    }

    #[test]
    fn test_auth_response_serialization() {
        let user = User {
            id: "user123".to_string(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            avatar: None,
            is_pro: false,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let auth_response = AuthResponse {
            user,
            access_token: "access_token_123".to_string(),
            refresh_token: "refresh_token_456".to_string(),
            expires_in: 3600,
        };

        let json = serde_json::to_string(&auth_response).unwrap();
        assert!(json.contains("access_token_123"));
        assert!(json.contains("refresh_token_456"));
        assert!(json.contains("3600"));
    }

    #[test]
    fn test_google_login_request_deserialization() {
        let json =
            r#"{"code":"auth_code_123","redirect_uri":"http://localhost:5173/auth/callback"}"#;
        let request: GoogleLoginRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.code, "auth_code_123");
        assert_eq!(request.redirect_uri, "http://localhost:5173/auth/callback");
    }

    #[test]
    fn test_update_profile_request_partial() {
        let json = r#"{"username":"newname"}"#;
        let request: UpdateUserProfileRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.username, Some("newname".to_string()));
        assert_eq!(request.avatar, None);
    }
}
