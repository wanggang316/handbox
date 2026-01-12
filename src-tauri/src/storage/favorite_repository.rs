// Favorite 数据访问层

use crate::models::AppError;
use crate::storage::types::{CreateFavoriteRequest, Favorite, FavoriteMessageType, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

#[derive(Clone)]
pub struct FavoriteRepository {
    db: Arc<Database>,
}

impl FavoriteRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn toggle_favorite(
        &self,
        request: &CreateFavoriteRequest,
    ) -> Result<bool, AppError> {
        let existing: Option<(String,)> = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM favorites WHERE message_id = $1",
        )
        .bind(&request.message_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to check favorite: {}", e)))?;

        if let Some((id,)) = existing {
            sqlx::query("DELETE FROM favorites WHERE id = $1")
                .bind(&id)
                .execute(self.db.pool())
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to delete favorite: {}", e)))?;
            return Ok(false);
        }

        let tags_json = if request.tags.is_empty() {
            None
        } else {
            serde_json::to_string(&request.tags).ok()
        };

        sqlx::query(
            r#"
                INSERT INTO favorites (
                    id, message_id, chat_id, content, role, message_type, tags, note, selected_text, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&request.message_id)
        .bind(&request.chat_id)
        .bind(&request.content)
        .bind(&request.role)
        .bind(format!("{:?}", request.message_type).to_lowercase())
        .bind(tags_json.as_deref())
        .bind(request.note.as_deref())
        .bind(request.selected_text.as_deref())
        .bind(request.created_at)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to create favorite: {}", e)))?;

        Ok(true)
    }

    pub async fn is_favorited(&self, message_id: &UUID) -> Result<bool, AppError> {
        let result: Option<(i64,)> = sqlx::query_as::<_, (i64,)>(
            "SELECT 1 FROM favorites WHERE message_id = $1",
        )
        .bind(message_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to check favorite: {}", e)))?;

        Ok(result.is_some())
    }

    pub async fn get_all_favorites(&self) -> Result<Vec<Favorite>, AppError> {
        let rows = sqlx::query(
            r#"
                SELECT id, message_id, chat_id, content, role, message_type, tags, note, selected_text, created_at
                FROM favorites
                ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get favorites: {}", e)))?;

        let mut favorites = Vec::new();
        for row in rows {
            favorites.push(Favorite {
                id: row.get("id"),
                message_id: row.get("message_id"),
                chat_id: row.get("chat_id"),
                content: row.get("content"),
                role: row.get("role"),
                message_type: FavoriteMessageType::from_str(&row.get::<String, _>("message_type")),
                tags: Favorite::tags_from_json(row.get::<Option<&str>, _>("tags")),
                note: row.get("note"),
                selected_text: row.get("selected_text"),
                created_at: row.get("created_at"),
            });
        }

        Ok(favorites)
    }

    pub async fn get_favorites_by_chat(&self, chat_id: &UUID) -> Result<Vec<Favorite>, AppError> {
        let rows = sqlx::query(
            r#"
                SELECT id, message_id, chat_id, content, role, message_type, tags, note, selected_text, created_at
                FROM favorites
                WHERE chat_id = $1
                ORDER BY created_at DESC
            "#,
        )
        .bind(chat_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get favorites: {}", e)))?;

        let mut favorites = Vec::new();
        for row in rows {
            favorites.push(Favorite {
                id: row.get("id"),
                message_id: row.get("message_id"),
                chat_id: row.get("chat_id"),
                content: row.get("content"),
                role: row.get("role"),
                message_type: FavoriteMessageType::from_str(&row.get::<String, _>("message_type")),
                tags: Favorite::tags_from_json(row.get::<Option<&str>, _>("tags")),
                note: row.get("note"),
                selected_text: row.get("selected_text"),
                created_at: row.get("created_at"),
            });
        }

        Ok(favorites)
    }

    pub async fn add_tag(&self, favorite_id: &UUID, tag: &str) -> Result<(), AppError> {
        let row = sqlx::query(
            "SELECT tags FROM favorites WHERE id = $1",
        )
        .bind(favorite_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get tags: {}", e)))?;

        let tags_json: Option<String> = row.get("tags");

        let mut tags: Vec<String> = Favorite::tags_from_json(tags_json.as_deref());
        if !tags.contains(&tag.to_string()) {
            tags.push(tag.to_string());
            let new_json = serde_json::to_string(&tags).map_err(|e| {
                AppError::internal_error(&format!("Failed to serialize tags: {}", e))
            })?;

            sqlx::query("UPDATE favorites SET tags = $1 WHERE id = $2")
                .bind(new_json.as_str())
                .bind(favorite_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to update tags: {}", e)))?;
        }

        Ok(())
    }

    pub async fn remove_tag(&self, favorite_id: &UUID, tag: &str) -> Result<(), AppError> {
        let row = sqlx::query(
            "SELECT tags FROM favorites WHERE id = $1",
        )
        .bind(favorite_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get tags: {}", e)))?;

        let tags_json: Option<String> = row.get("tags");

        let mut tags: Vec<String> = Favorite::tags_from_json(tags_json.as_deref());
        tags.retain(|t| t != tag);

        let new_json = serde_json::to_string(&tags).map_err(|e| {
            AppError::internal_error(&format!("Failed to serialize tags: {}", e))
        })?;

        sqlx::query("UPDATE favorites SET tags = $1 WHERE id = $2")
            .bind(new_json.as_str())
            .bind(favorite_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update tags: {}", e)))?;

        Ok(())
    }

    pub async fn delete_by_message_id(&self, message_id: &UUID) -> Result<(), AppError> {
        sqlx::query("DELETE FROM favorites WHERE message_id = $1")
            .bind(message_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete favorite: {}", e)))?;

        Ok(())
    }
}
