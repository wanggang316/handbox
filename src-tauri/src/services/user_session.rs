use crate::models::{AppError, User};
use crate::storage::Database;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub user: User,
    pub google_access_token: String,
    pub google_refresh_token: String,
    pub token_expires_at: i64, // Unix timestamp
}

#[derive(Clone)]
pub struct UserSessionService {
    db: Arc<Database>,
    current_session: Arc<RwLock<Option<UserSession>>>,
}

impl UserSessionService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            current_session: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn create_session(
        &self,
        user: User,
        google_access_token: String,
        google_refresh_token: String,
        expires_in: u64,
    ) -> Result<(), AppError> {
        self.save_user_to_db(&user).await?;

        let now = chrono::Utc::now().timestamp();
        let token_expires_at = now + expires_in as i64;

        let session = UserSession {
            user: user.clone(),
            google_access_token: google_access_token.clone(),
            google_refresh_token: google_refresh_token.clone(),
            token_expires_at,
        };

        let mut current = self.current_session.write().await;
        *current = Some(session.clone());

        self.save_session_to_db(&session).await?;

        tracing::info!(
            "用户会话创建成功: user_id={}, expires_at={}",
            user.id,
            token_expires_at
        );

        Ok(())
    }

    pub async fn get_current_session(&self) -> Option<UserSession> {
        self.current_session.read().await.clone()
    }

    pub async fn get_current_user(&self) -> Option<User> {
        self.current_session
            .read()
            .await
            .as_ref()
            .map(|s| s.user.clone())
    }

    pub async fn update_user(&self, user: &User) -> Result<(), AppError> {
        self.save_user_to_db(user).await?;

        let mut session = self.current_session.write().await;
        if let Some(ref mut s) = *session {
            s.user = user.clone();
        }

        tracing::info!("用户信息已更新: user_id={}", user.id);
        Ok(())
    }

    pub async fn is_session_valid(&self) -> bool {
        if let Some(session) = self.current_session.read().await.as_ref() {
            let now = chrono::Utc::now().timestamp();
            return session.token_expires_at > now;
        }
        false
    }

    pub async fn clear_session(&self) -> Result<(), AppError> {
        let mut current = self.current_session.write().await;
        if let Some(session) = current.take() {
            tracing::info!("清除用户会话: user_id={}", session.user.id);
        }
        Ok(())
    }

    /// Restores the most recently active session at app startup.
    pub async fn load_session_from_db(&self) -> Result<(), AppError> {
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
            let now = chrono::Utc::now().timestamp();
            if token_expires_at <= now {
                tracing::warn!(
                    "会话已过期: user_id={}, expired_at={}",
                    id,
                    token_expires_at
                );
                // Refresh tokens are not persisted, so an expired session cannot be
                // renewed here; the user has to sign in again.
                return Ok(());
            }

            let user = User {
                id: id.clone(),
                username,
                email,
                avatar,
                is_pro,
                created_at,
                updated_at,
            };

            // Placeholder session carrying no tokens: they belong in the OS keychain.
            let session = UserSession {
                user: user.clone(),
                google_access_token: String::new(), // TODO: read from OS keychain
                google_refresh_token: String::new(), // TODO: read from OS keychain
                token_expires_at,
            };

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

        // TODO: exchange the refresh token with Google for a new access token

        Ok(())
    }

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

    /// Persists session metadata only; tokens still need OS keychain storage.
    async fn save_session_to_db(&self, session: &UserSession) -> Result<(), AppError> {
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
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        // TODO: implement
    }

    #[tokio::test]
    async fn test_session_validation() {
        // TODO: implement
    }
}
