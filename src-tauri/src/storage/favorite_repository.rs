// Favorite 数据访问层

use crate::models::AppError;
use crate::storage::types::{CreateFavoriteRequest, Favorite, FavoriteMessageType, FavoriteTag, UUID};
use crate::storage::Database;
use sqlx::{Error, Row};
use std::collections::HashMap;
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
        // 检查该chat_id和message_type的组合是否已存在
        let existing: Option<(String, String)> = sqlx::query_as::<_, (String, String)>(
            "SELECT id, message_type FROM favorites WHERE chat_id = $1 AND message_type = $2",
        )
        .bind(&request.chat_id)
        .bind(format!("{:?}", request.message_type).to_lowercase())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to check favorite: {}", e)))?;

        if let Some((id, _)) = existing {
            sqlx::query("DELETE FROM favorites WHERE id = $1")
                .bind(&id)
                .execute(self.db.pool())
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to delete favorite: {}", e)))?;
            return Ok(false);
        }

        let favorite_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            r#"
                INSERT INTO favorites (
                    id, message_id, chat_id, content, role, message_type, note, context, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(&favorite_id)
        .bind(&request.message_id)
        .bind(&request.chat_id)
        .bind(&request.content)
        .bind(&request.role)
        .bind(format!("{:?}", request.message_type).to_lowercase())
        .bind(request.note.as_deref())
        .bind(request.context.as_deref())
        .bind(request.created_at)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to create favorite: {}", e)))?;

        if !request.tags.is_empty() {
            self.attach_tags_to_favorite(&favorite_id, &request.tags)
                .await?;
        }

        Ok(true)
    }

    pub async fn is_favorited(
        &self,
        message_id: &UUID,
        chat_id: &UUID,
        message_type: &FavoriteMessageType,
    ) -> Result<bool, AppError> {
        let result: Option<(i64,)> = sqlx::query_as::<_, (i64,)>(
            "SELECT 1 FROM favorites WHERE message_id = $1 AND chat_id = $2 AND message_type = $3",
        )
        .bind(message_id)
        .bind(chat_id)
        .bind(format!("{:?}", message_type).to_lowercase())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e: Error| AppError::internal_error(&format!("Failed to check favorite: {}", e)))?;

        Ok(result.is_some())
    }

    pub async fn get_all_favorites(&self) -> Result<Vec<Favorite>, AppError> {
        let rows = sqlx::query(
            r#"
                SELECT id, message_id, chat_id, content, role, message_type, note, context, created_at
                FROM favorites
                ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|e: Error| AppError::internal_error(&format!("Failed to get favorites: {}", e)))?;

        let mut favorites = Vec::new();
        for row in rows {
            favorites.push(Favorite {
                id: row.get("id"),
                message_id: row.get("message_id"),
                chat_id: row.get("chat_id"),
                content: row.get("content"),
                role: row.get("role"),
                message_type: FavoriteMessageType::from_str(&row.get::<String, _>("message_type")),
                tags: vec![],
                note: row.get("note"),
                context: row.get("context"),
                created_at: row.get("created_at"),
            });
        }

        self.attach_favorite_tags(&mut favorites).await?;
        Ok(favorites)
    }

    pub async fn get_favorites_by_chat(&self, chat_id: &UUID) -> Result<Vec<Favorite>, AppError> {
        let rows = sqlx::query(
            r#"
                SELECT id, message_id, chat_id, content, role, message_type, note, context, created_at
                FROM favorites
                WHERE chat_id = $1
                ORDER BY created_at DESC
            "#,
        )
        .bind(chat_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e: Error| AppError::internal_error(&format!("Failed to get favorites: {}", e)))?;

        let mut favorites = Vec::new();
        for row in rows {
            favorites.push(Favorite {
                id: row.get("id"),
                message_id: row.get("message_id"),
                chat_id: row.get("chat_id"),
                content: row.get("content"),
                role: row.get("role"),
                message_type: FavoriteMessageType::from_str(&row.get::<String, _>("message_type")),
                tags: vec![],
                note: row.get("note"),
                context: row.get("context"),
                created_at: row.get("created_at"),
            });
        }

        self.attach_favorite_tags(&mut favorites).await?;
        Ok(favorites)
    }

    pub async fn add_tag(&self, favorite_id: &UUID, tag: &FavoriteTag) -> Result<(), AppError> {
        self.attach_tags_to_favorite(favorite_id, &[tag.clone()])
            .await
    }

    pub async fn remove_tag(&self, favorite_id: &UUID, tag_name: &str) -> Result<(), AppError> {
        let tag_id: Option<String> = sqlx::query_scalar("SELECT id FROM tags WHERE name = $1")
            .bind(tag_name)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to find tag: {}", e)))?;

        let Some(tag_id) = tag_id else {
            return Ok(());
        };

        sqlx::query("DELETE FROM favorite_tags WHERE favorite_id = $1 AND tag_id = $2")
            .bind(favorite_id)
            .bind(tag_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to remove tag: {}", e)))?;

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

    pub async fn upsert_text_favorite(
        &self,
        request: &CreateFavoriteRequest,
    ) -> Result<(), AppError> {
        let existing_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM favorites WHERE message_id = $1 AND chat_id = $2 AND message_type = $3",
        )
        .bind(&request.message_id)
        .bind(&request.chat_id)
        .bind(format!("{:?}", request.message_type).to_lowercase())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to check favorite: {}", e)))?;

        if let Some(id) = existing_id {
            sqlx::query(
                r#"
                    UPDATE favorites
                    SET content = $1, role = $2, note = $3, context = $4
                    WHERE id = $5
                "#,
            )
            .bind(&request.content)
            .bind(&request.role)
            .bind(request.note.as_deref())
            .bind(request.context.as_deref())
            .bind(&id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update favorite: {}", e)))?;
            if !request.tags.is_empty() {
                self.attach_tags_to_favorite(&id, &request.tags).await?;
            }
        } else {
            let favorite_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                    INSERT INTO favorites (
                        id, message_id, chat_id, content, role, message_type, note, context, created_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(&favorite_id)
            .bind(&request.message_id)
            .bind(&request.chat_id)
            .bind(&request.content)
            .bind(&request.role)
            .bind(format!("{:?}", request.message_type).to_lowercase())
            .bind(request.note.as_deref())
            .bind(request.context.as_deref())
            .bind(request.created_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create favorite: {}", e)))?;

            if !request.tags.is_empty() {
                self.attach_tags_to_favorite(&favorite_id, &request.tags)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn delete_text_favorite(
        &self,
        message_id: &UUID,
        chat_id: &UUID,
    ) -> Result<(), AppError> {
        sqlx::query(
            "DELETE FROM favorites WHERE message_id = $1 AND chat_id = $2 AND message_type = $3",
        )
        .bind(message_id)
        .bind(chat_id)
        .bind("text")
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to delete favorite: {}", e)))?;

        Ok(())
    }

    pub async fn list_tags(&self) -> Result<Vec<FavoriteTag>, AppError> {
        let rows = sqlx::query("SELECT name, color FROM tags ORDER BY name ASC")
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list tags: {}", e)))?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(FavoriteTag {
                name: row.get("name"),
                color: row.get("color"),
            });
        }

        Ok(tags)
    }

    async fn upsert_tag(&self, tag: &FavoriteTag) -> Result<UUID, AppError> {
        let existing: Option<(String, String)> =
            sqlx::query_as("SELECT id, color FROM tags WHERE name = $1")
                .bind(&tag.name)
                .fetch_optional(self.db.pool())
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to find tag: {}", e)))?;

        if let Some((id, color)) = existing {
            if color != tag.color {
                sqlx::query("UPDATE tags SET color = $1 WHERE id = $2")
                    .bind(&tag.color)
                    .bind(&id)
                    .execute(self.db.pool())
                    .await
                    .map_err(|e| AppError::internal_error(&format!("Failed to update tag: {}", e)))?;
            }
            return Ok(id);
        }

        let tag_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO tags (id, name, color, created_at) VALUES ($1, $2, $3, $4)")
            .bind(&tag_id)
            .bind(&tag.name)
            .bind(&tag.color)
            .bind(chrono::Utc::now().timestamp_millis())
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create tag: {}", e)))?;

        Ok(tag_id)
    }

    async fn attach_tags_to_favorite(
        &self,
        favorite_id: &UUID,
        tags: &[FavoriteTag],
    ) -> Result<(), AppError> {
        for tag in tags {
            let tag_id = self.upsert_tag(tag).await?;
            sqlx::query(
                "INSERT OR IGNORE INTO favorite_tags (favorite_id, tag_id) VALUES ($1, $2)",
            )
            .bind(favorite_id)
            .bind(tag_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to add tag: {}", e)))?;
        }

        Ok(())
    }

    async fn attach_favorite_tags(&self, favorites: &mut [Favorite]) -> Result<(), AppError> {
        if favorites.is_empty() {
            return Ok(());
        }

        let favorite_ids: Vec<String> = favorites
            .iter()
            .map(|favorite| favorite.id.clone())
            .collect();
        let tags_map = self.get_tags_by_favorite_ids(&favorite_ids).await?;

        for favorite in favorites.iter_mut() {
            if let Some(tags) = tags_map.get(&favorite.id) {
                favorite.tags = tags.clone();
            }
        }

        Ok(())
    }

    async fn get_tags_by_favorite_ids(
        &self,
        favorite_ids: &[UUID],
    ) -> Result<HashMap<UUID, Vec<FavoriteTag>>, AppError> {
        let mut map: HashMap<UUID, Vec<FavoriteTag>> = HashMap::new();
        if favorite_ids.is_empty() {
            return Ok(map);
        }

        let mut query = String::from(
            "SELECT ft.favorite_id as favorite_id, t.name as name, t.color as color FROM favorite_tags ft JOIN tags t ON t.id = ft.tag_id WHERE ft.favorite_id IN (",
        );
        for (index, _) in favorite_ids.iter().enumerate() {
            if index > 0 {
                query.push(',');
            }
            query.push_str(&format!("${}", index + 1));
        }
        query.push(')');

        let mut sql = sqlx::query(&query);
        for favorite_id in favorite_ids {
            sql = sql.bind(favorite_id);
        }

        let rows = sql
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to load tags: {}", e)))?;

        for row in rows {
            let favorite_id: String = row.get("favorite_id");
            let name: String = row.get("name");
            let color: String = row.get("color");
            map.entry(favorite_id)
                .or_insert_with(Vec::new)
                .push(FavoriteTag { name, color });
        }

        Ok(map)
    }
}
