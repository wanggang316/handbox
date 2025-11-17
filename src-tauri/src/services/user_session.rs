use crate::models::{AppError, User};
use crate::storage::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 用户会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub user: User,
    pub google_access_token: String,
    pub google_refresh_token: String,
    pub token_expires_at: i64, // Unix timestamp
}

/// 用户会话管理服务
#[derive(Clone)]
pub struct UserSessionService {
    db: Arc<Database>,
    current_session: Arc<RwLock<Option<UserSession>>>,
}

impl UserSessionService {
    /// 创建新的会话服务
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            current_session: Arc::new(RwLock::new(None)),
        }
    }

    /// 创建或更新用户会话
    pub async fn create_session(
        &self,
        user: User,
        google_access_token: String,
        google_refresh_token: String,
        expires_in: u64,
    ) -> Result<(), AppError> {
        // 1. 保存或更新用户到数据库
        self.save_user_to_db(&user).await?;

        // 2. 计算 token 过期时间
        let now = chrono::Utc::now().timestamp();
        let token_expires_at = now + expires_in as i64;

        // 3. 创建会话
        let session = UserSession {
            user: user.clone(),
            google_access_token: google_access_token.clone(),
            google_refresh_token: google_refresh_token.clone(),
            token_expires_at,
        };

        // 4. 保存到内存
        let mut current = self.current_session.write().await;
        *current = Some(session.clone());

        // 5. 持久化会话信息（加密存储 token）
        self.save_session_to_db(&session).await?;

        tracing::info!(
            "用户会话创建成功: user_id={}, expires_at={}",
            user.id,
            token_expires_at
        );

        Ok(())
    }

    /// 获取当前会话
    pub async fn get_current_session(&self) -> Option<UserSession> {
        self.current_session.read().await.clone()
    }

    /// 获取当前用户
    pub async fn get_current_user(&self) -> Option<User> {
        self.current_session
            .read()
            .await
            .as_ref()
            .map(|s| s.user.clone())
    }

    /// 更新用户信息
    pub async fn update_user(&self, user: &User) -> Result<(), AppError> {
        // 1. 更新数据库
        self.save_user_to_db(user).await?;

        // 2. 更新内存中的会话
        let mut session = self.current_session.write().await;
        if let Some(ref mut s) = *session {
            s.user = user.clone();
        }

        tracing::info!("用户信息已更新: user_id={}", user.id);
        Ok(())
    }

    /// 检查会话是否有效
    pub async fn is_session_valid(&self) -> bool {
        if let Some(session) = self.current_session.read().await.as_ref() {
            let now = chrono::Utc::now().timestamp();
            return session.token_expires_at > now;
        }
        false
    }

    /// 清除会话（登出）
    pub async fn clear_session(&self) -> Result<(), AppError> {
        let mut current = self.current_session.write().await;
        if let Some(session) = current.take() {
            tracing::info!("清除用户会话: user_id={}", session.user.id);
            // 可选：从数据库删除会话
            // self.delete_session_from_db(&session.user.id).await?;
        }
        Ok(())
    }

    /// 从数据库加载会话（应用启动时）
    pub async fn load_session_from_db(&self) -> Result<(), AppError> {
        // 1. 查询最后一次活跃的会话
        let query = r#"
            SELECT
                u.id, u.username, u.email, u.avatar, u.is_pro, u.created_at, u.updated_at,
                s.token_expires_at
            FROM users u
            INNER JOIN user_sessions s ON u.id = s.user_id
            ORDER BY s.updated_at DESC
            LIMIT 1
        "#;

        let result = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                bool,
                String,
                String,
                i64,
            ),
        >(query)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError {
            code: "DATABASE_ERROR".to_string(),
            message: format!("加载会话失败: {}", e),
            hint: None,
        })?;

        if let Some((
            id,
            username,
            email,
            avatar,
            is_pro,
            created_at,
            updated_at,
            token_expires_at,
        )) = result
        {
            // 2. 检查 token 是否过期
            let now = chrono::Utc::now().timestamp();
            if token_expires_at <= now {
                tracing::warn!(
                    "会话已过期: user_id={}, expired_at={}",
                    id,
                    token_expires_at
                );
                // Token 已过期，清除会话
                // 注意：由于我们没有存储 refresh_token（应该用 OS Keychain 存储）
                // 这里无法自动刷新，需要用户重新登录
                return Ok(());
            }

            // 3. 重建用户对象
            let user = User {
                id: id.clone(),
                username,
                email,
                avatar,
                is_pro,
                created_at,
                updated_at,
            };

            // 4. 创建一个临时会话（没有 token，因为 token 应该用 Keychain 存储）
            // 这里我们暂时创建一个占位符会话，表示用户已登录但 token 已过期
            let session = UserSession {
                user: user.clone(),
                google_access_token: String::new(), // TODO: 从 OS Keychain 读取
                google_refresh_token: String::new(), // TODO: 从 OS Keychain 读取
                token_expires_at,
            };

            // 5. 恢复到内存
            let mut current = self.current_session.write().await;
            *current = Some(session);

            tracing::info!(
                "会话已从数据库恢复: user_id={}, expires_at={}",
                id,
                token_expires_at
            );
        } else {
            tracing::info!("数据库中没有历史会话");
        }

        Ok(())
    }

    /// 刷新 Google Access Token
    pub async fn refresh_google_token(&self) -> Result<(), AppError> {
        let _session = self
            .current_session
            .read()
            .await
            .clone()
            .ok_or_else(|| AppError {
                code: "NO_SESSION".to_string(),
                message: "当前没有活跃会话".to_string(),
                hint: Some("请先登录".to_string()),
            })?;

        // TODO: 实现 Google token 刷新逻辑
        // 使用 refresh_token 向 Google 请求新的 access_token

        Ok(())
    }

    /// 保存用户到数据库
    async fn save_user_to_db(&self, user: &User) -> Result<(), AppError> {
        let query = r#"
            INSERT INTO users (id, username, email, avatar, is_pro, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                username = excluded.username,
                email = excluded.email,
                avatar = excluded.avatar,
                is_pro = excluded.is_pro,
                updated_at = excluded.updated_at
        "#;

        sqlx::query(query)
            .bind(&user.id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(&user.avatar)
            .bind(user.is_pro)
            .bind(&user.created_at)
            .bind(&user.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError {
                code: "DATABASE_ERROR".to_string(),
                message: format!("保存用户信息失败: {}", e),
                hint: None,
            })?;

        tracing::info!("用户信息已保存到数据库: {}", user.id);
        Ok(())
    }

    /// 保存会话到数据库（加密存储）
    async fn save_session_to_db(&self, session: &UserSession) -> Result<(), AppError> {
        // TODO: 实现会话持久化
        // 1. 使用 OS Keychain 存储敏感的 token
        // 2. 或者使用加密后存储到数据库
        // 3. 保存会话元数据

        let query = r#"
            INSERT INTO user_sessions (user_id, token_expires_at, created_at)
            VALUES (?, ?, datetime('now'))
            ON CONFLICT(user_id) DO UPDATE SET
                token_expires_at = excluded.token_expires_at,
                updated_at = datetime('now')
        "#;

        sqlx::query(query)
            .bind(&session.user.id)
            .bind(session.token_expires_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError {
                code: "DATABASE_ERROR".to_string(),
                message: format!("保存会话失败: {}", e),
                hint: None,
            })?;

        tracing::info!("会话信息已保存到数据库: user_id={}", session.user.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        // TODO: 实现测试
    }

    #[tokio::test]
    async fn test_session_validation() {
        // TODO: 实现测试
    }
}
