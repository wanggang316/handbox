// GenUI 数据访问层

use crate::models::AppError;
use crate::storage::types::{GenUi, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// GenUI 仓储层
#[derive(Clone)]
pub struct GenUiRepository {
    db: Arc<Database>,
}

impl GenUiRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建 GenUI
    pub async fn create_genui(&self, genui: &GenUi) -> Result<(), AppError> {
        let query = r#"
            INSERT INTO genui (id, name, spec, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
        "#;

        sqlx::query(query)
            .bind(&genui.id)
            .bind(&genui.name)
            .bind(&genui.spec)
            .bind(genui.created_at)
            .bind(genui.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create genui: {}", e)))?;

        Ok(())
    }

    /// 获取 GenUI 列表
    pub async fn list_genui(&self, limit: i32, offset: i32) -> Result<Vec<GenUi>, AppError> {
        let query = r#"
            SELECT id, name, spec, created_at, updated_at
            FROM genui ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list genui: {}", e)))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(Self::row_to_genui(row)?);
        }

        Ok(items)
    }

    /// 根据 ID 获取 GenUI
    pub async fn get_genui_by_id(&self, id: &UUID) -> Result<Option<GenUi>, AppError> {
        let query = r#"
            SELECT id, name, spec, created_at, updated_at
            FROM genui WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get genui: {}", e)))?;

        match row {
            Some(row) => Ok(Some(Self::row_to_genui(row)?)),
            None => Ok(None),
        }
    }

    /// 更新 GenUI（名称 + spec）
    pub async fn update_genui(&self, genui: &GenUi) -> Result<(), AppError> {
        let query = r#"
            UPDATE genui SET name = $1, spec = $2, updated_at = $3
            WHERE id = $4
        "#;

        let result = sqlx::query(query)
            .bind(&genui.name)
            .bind(&genui.spec)
            .bind(genui.updated_at)
            .bind(&genui.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update genui: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "GenUI not found: {}",
                genui.id
            )));
        }

        Ok(())
    }

    /// 删除 GenUI。先把所有引用它的 agent 的 `genui_id` 置空，避免悬挂引用，再删除本体。
    pub async fn delete_genui(&self, id: &UUID) -> Result<(), AppError> {
        sqlx::query("UPDATE agents SET genui_id = NULL WHERE genui_id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to clear genui references: {}", e))
            })?;

        let result = sqlx::query("DELETE FROM genui WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete genui: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("GenUI not found: {}", id)));
        }

        Ok(())
    }

    // 辅助方法：将数据库行转换为 GenUi
    fn row_to_genui(row: sqlx::sqlite::SqliteRow) -> Result<GenUi, AppError> {
        Ok(GenUi {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            spec: row.try_get("spec")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path).await.unwrap();
        (db_service, temp_dir)
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    #[tokio::test]
    async fn test_genui_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = GenUiRepository::new(Arc::new(db));
        let now = now_ms();

        let genui = GenUi {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Translation Card".to_string(),
            spec: r#"{"root":"card","elements":{"card":{"type":"Card","props":{},"children":[],"visible":true}}}"#.to_string(),
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_genui(&genui).await.unwrap();

        // Get by ID
        let fetched = repo.get_genui_by_id(&genui.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, genui.name);
        assert_eq!(fetched.spec, genui.spec);

        // List
        let items = repo.list_genui(10, 0).await.unwrap();
        assert_eq!(items.len(), 1);

        // Update
        let mut updated = genui.clone();
        updated.name = "Renamed Card".to_string();
        updated.spec = r#"{"root":"x","elements":{}}"#.to_string();
        updated.updated_at = now + 1000;
        repo.update_genui(&updated).await.unwrap();

        let after = repo.get_genui_by_id(&genui.id).await.unwrap().unwrap();
        assert_eq!(after.name, "Renamed Card");
        assert_eq!(after.spec, r#"{"root":"x","elements":{}}"#);

        // Delete
        repo.delete_genui(&genui.id).await.unwrap();
        assert!(repo.get_genui_by_id(&genui.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_update_missing_genui_is_not_found() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = GenUiRepository::new(Arc::new(db));
        let now = now_ms();

        let ghost = GenUi {
            id: "does-not-exist".to_string(),
            name: "Ghost".to_string(),
            spec: "{}".to_string(),
            created_at: now,
            updated_at: now,
        };

        let err = repo
            .update_genui(&ghost)
            .await
            .expect_err("expected not found");
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// Deleting a GenUI must null out any agent that referenced it, so no agent
    /// is left pointing at a deleted spec.
    #[tokio::test]
    async fn test_delete_genui_clears_agent_reference() {
        let (db, _temp_dir) = create_test_db().await;
        let db = Arc::new(db);
        let repo = GenUiRepository::new(db.clone());
        let now = now_ms();

        let genui = GenUi {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Card".to_string(),
            spec: "{}".to_string(),
            created_at: now,
            updated_at: now,
        };
        repo.create_genui(&genui).await.unwrap();

        // Seed an agent row referencing the genui (raw insert: only the columns we need).
        let agent_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agents (id, name, genui_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&agent_id)
        .bind("Linked Agent")
        .bind(&genui.id)
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        repo.delete_genui(&genui.id).await.unwrap();

        let dangling: Option<String> = sqlx::query("SELECT genui_id FROM agents WHERE id = ?")
            .bind(&agent_id)
            .fetch_one(db.pool())
            .await
            .unwrap()
            .try_get("genui_id")
            .unwrap();
        assert_eq!(
            dangling, None,
            "agent.genui_id should be NULL after the genui is deleted"
        );
    }
}
