// 聊天服务实现

use crate::models::{AppError, Chat, UUID};
use crate::services::DatabaseService;
use std::sync::Arc;

/// 聊天服务
pub struct ChatService {
    db: Arc<DatabaseService>,
}

impl ChatService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self { db }
    }

    /// 创建聊天
    pub async fn create_chat(
        &self,
        name: String,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<String>>,
    ) -> Result<Chat, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let chat_id = uuid::Uuid::new_v4().to_string();
        let mcp_servers = mcp_servers.unwrap_or_default();
        let mcp_servers_json = serde_json::to_string(&mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let chat = Chat {
            id: chat_id.clone(),
            name,
            last_message_at: None,
            message_count: 0,
            system_prompt,
            mcp_servers,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // 保存到数据库
        let pool = self.db.pool();
        sqlx::query(
            "INSERT INTO chats (id, name, system_prompt, mcp_servers, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )
        .bind(&chat.id)
        .bind(&chat.name)
        .bind(&chat.system_prompt)
        .bind(&mcp_servers_json)
        .bind(chat.created_at)
        .bind(chat.updated_at)
        .execute(pool)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to create chat: {}", e)))?;

        Ok(chat)
    }

    /// 获取聊天列表
    pub async fn list_chats(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Chat>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let pool = self.db.pool();
        let rows = sqlx::query_as::<_, (String, String, Option<i64>, i32, Option<String>, Option<String>, Option<String>, i64, i64)>(
            "SELECT id, name, last_message_at, message_count, system_prompt, mcp_servers, artifact_id, created_at, updated_at FROM chats ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to list chats: {}", e)))?;

        let mut chats = Vec::new();
        for row in rows {
            let mcp_servers: Vec<String> = if let Some(json) = &row.5 {
                serde_json::from_str(json).unwrap_or_default()
            } else {
                Vec::new()
            };

            chats.push(Chat {
                id: row.0,
                name: row.1,
                last_message_at: row.2,
                message_count: row.3,
                system_prompt: row.4,
                mcp_servers,
                artifact_id: row.6,
                created_at: row.7,
                updated_at: row.8,
            });
        }

        Ok(chats)
    }

    /// 获取聊天详情
    pub async fn get_chat(&self, chat_id: UUID) -> Result<Chat, AppError> {
        let pool = self.db.pool();
        let row = sqlx::query_as::<_, (String, String, Option<i64>, i32, Option<String>, Option<String>, Option<String>, i64, i64)>(
            "SELECT id, name, last_message_at, message_count, system_prompt, mcp_servers, artifact_id, created_at, updated_at FROM chats WHERE id = ?1"
        )
        .bind(&chat_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get chat: {}", e)))?;

        match row {
            Some(row) => {
                let mcp_servers: Vec<String> = if let Some(json) = &row.5 {
                    serde_json::from_str(json).unwrap_or_default()
                } else {
                    Vec::new()
                };

                Ok(Chat {
                    id: row.0,
                    name: row.1,
                    last_message_at: row.2,
                    message_count: row.3,
                    system_prompt: row.4,
                    mcp_servers,
                    artifact_id: row.6,
                    created_at: row.7,
                    updated_at: row.8,
                })
            }
            None => Err(AppError::not_found(&format!("Chat not found: {}", chat_id))),
        }
    }

    /// 更新聊天
    pub async fn update_chat(
        &self,
        chat_id: UUID,
        name: Option<String>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<String>>,
    ) -> Result<Chat, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let pool = self.db.pool();

        // 先检查聊天是否存在
        let existing_chat = self.get_chat(chat_id.clone()).await?;

        // 构建更新字段
        let updated_name = name.unwrap_or(existing_chat.name);
        let updated_system_prompt = system_prompt.or(existing_chat.system_prompt);
        let updated_mcp_servers = mcp_servers.unwrap_or(existing_chat.mcp_servers);
        let mcp_servers_json = serde_json::to_string(&updated_mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        sqlx::query(
            "UPDATE chats SET name = ?1, system_prompt = ?2, mcp_servers = ?3, updated_at = ?4 WHERE id = ?5"
        )
        .bind(&updated_name)
        .bind(&updated_system_prompt)
        .bind(&mcp_servers_json)
        .bind(now)
        .bind(&chat_id)
        .execute(pool)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to update chat: {}", e)))?;

        // 返回更新后的聊天
        Ok(Chat {
            id: existing_chat.id,
            name: updated_name,
            last_message_at: existing_chat.last_message_at,
            message_count: existing_chat.message_count,
            system_prompt: updated_system_prompt,
            mcp_servers: updated_mcp_servers,
            artifact_id: existing_chat.artifact_id,
            created_at: existing_chat.created_at,
            updated_at: now,
        })
    }

    /// 删除聊天
    pub async fn delete_chat(&self, chat_id: UUID) -> Result<(), AppError> {
        let pool = self.db.pool();

        // 先检查聊天是否存在
        self.get_chat(chat_id.clone()).await?;

        // 删除聊天（相关消息会通过外键级联删除）
        let result = sqlx::query("DELETE FROM chats WHERE id = ?1")
            .bind(&chat_id)
            .execute(pool)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to delete chat: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Chat not found: {}", chat_id)));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;