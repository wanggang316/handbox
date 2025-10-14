// Provider 数据访问层 - 使用普通查询避免 sqlx 宏问题

use crate::models::AppError;
use crate::storage::types::{Provider, ProviderWithModels};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Provider 仓储层
#[derive(Clone)]
pub struct ProviderRepository {
    db: Arc<Database>,
}

impl ProviderRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建供应商
    pub async fn create_provider(&self, provider: &Provider) -> Result<(), AppError> {
        let query = r#"
            INSERT INTO providers (id, name, provider_type, base_url, api_key, enabled,
                                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#;

        sqlx::query(query)
            .bind(&provider.id)
            .bind(&provider.name)
            .bind(&provider.provider_type)
            .bind(&provider.base_url)
            .bind(&provider.api_key)
            .bind(provider.enabled)
            .bind(provider.created_at)
            .bind(provider.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint failed") {
                    AppError::provider_name_exists()
                } else {
                    AppError::internal_error(&format!("Failed to create provider: {}", e))
                }
            })?;

        Ok(())
    }

    /// 更新供应商
    pub async fn update_provider(&self, provider: &Provider) -> Result<(), AppError> {
        tracing::debug!(
            "Updating provider in database: ID={}, Name={}",
            provider.id,
            provider.name
        );

        let query = r#"
            UPDATE providers SET
                name = $2, provider_type = $3, base_url = $4, api_key = $5,
                enabled = $6, updated_at = $7
            WHERE id = $1
        "#;

        let result = sqlx::query(query)
            .bind(&provider.id)
            .bind(&provider.name)
            .bind(&provider.provider_type)
            .bind(&provider.base_url)
            .bind(&provider.api_key)
            .bind(provider.enabled)
            .bind(provider.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                if e.to_string().contains("UNIQUE constraint failed") {
                    AppError::provider_name_exists()
                } else {
                    AppError::internal_error(&format!("Failed to update provider: {}", e))
                }
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Provider not found"));
        }

        Ok(())
    }

    /// 根据 ID 获取供应商
    pub async fn get_provider_by_id(&self, id: &str) -> Result<Option<Provider>, AppError> {
        let query = r#"
            SELECT id, name, provider_type, base_url, api_key, enabled,
                   created_at, updated_at
            FROM providers WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get provider: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_provider(row)?))
        } else {
            Ok(None)
        }
    }

    /// 根据名称获取供应商
    pub async fn get_provider_by_name(&self, name: &str) -> Result<Option<Provider>, AppError> {
        let query = r#"
            SELECT id, name, provider_type, base_url, api_key, enabled,
                   created_at, updated_at
            FROM providers WHERE name = $1
        "#;

        let row = sqlx::query(query)
            .bind(name)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get provider by name: {}", e))
            })?;

        if let Some(row) = row {
            Ok(Some(self.row_to_provider(row)?))
        } else {
            Ok(None)
        }
    }

    /// 获取所有供应商
    pub async fn list_providers(&self) -> Result<Vec<Provider>, AppError> {
        let query = r#"
            SELECT id, name, provider_type, base_url, api_key, enabled,
                   created_at, updated_at
            FROM providers ORDER BY created_at
        "#;

        let rows = sqlx::query(query)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list providers: {}", e)))?;

        let mut providers = Vec::new();
        for row in rows {
            providers.push(self.row_to_provider(row)?);
        }

        tracing::info!("Providers: {:?}", providers);
        Ok(providers)
    }

    /// 获取带模型的供应商
    /// 注意：此方法依赖 ModelRepository，应该在服务层组合使用
    pub async fn get_provider_with_models(
        &self,
        id: &str,
        model_repo: &crate::storage::ModelRepository,
    ) -> Result<Option<ProviderWithModels>, AppError> {
        let provider = self.get_provider_by_id(id).await?;

        match provider {
            Some(p) => {
                let models = model_repo.get_models_by_provider(id).await?;
                Ok(Some(ProviderWithModels {
                    id: p.id,
                    name: p.name,
                    provider_type: p.provider_type,
                    base_url: p.base_url,
                    api_key: p.api_key,
                    enabled: p.enabled,
                    models,
                    created_at: p.created_at,
                    updated_at: p.updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// 删除供应商
    pub async fn delete_provider(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM providers WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete provider: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Provider not found"));
        }

        Ok(())
    }

    // 辅助方法：将数据库行转换为 Provider
    fn row_to_provider(&self, row: sqlx::sqlite::SqliteRow) -> Result<Provider, AppError> {
        Ok(Provider {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            provider_type: row.try_get("provider_type")?,
            base_url: row.try_get("base_url")?,
            api_key: row.try_get("api_key")?,
            enabled: row.try_get("enabled")?,
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

    #[tokio::test]
    async fn test_provider_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ProviderRepository::new(Arc::new(db));

        let now = chrono::Utc::now().timestamp_millis();
        let provider = Provider {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Provider".to_string(),
            provider_type: "openai".to_string(),
            base_url: "https://api.openai.com".to_string(),
            api_key: "test-api-key".to_string(),
            enabled: true,
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_provider(&provider).await.unwrap();

        // Get by ID
        let fetched = repo.get_provider_by_id(&provider.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.as_ref().unwrap().name, provider.name);

        // Get by name
        let fetched_by_name = repo.get_provider_by_name(&provider.name).await.unwrap();
        assert!(fetched_by_name.is_some());
        assert_eq!(fetched_by_name.as_ref().unwrap().id, provider.id);

        // List - includes 1 test provider (no predefined providers)
        let providers = repo.list_providers().await.unwrap();
        assert_eq!(providers.len(), 1);

        // Update
        let mut updated_provider = provider.clone();
        updated_provider.name = "Updated Provider".to_string();
        updated_provider.updated_at = chrono::Utc::now().timestamp_millis();

        repo.update_provider(&updated_provider).await.unwrap();

        let fetched_updated = repo.get_provider_by_id(&provider.id).await.unwrap();
        assert_eq!(fetched_updated.unwrap().name, "Updated Provider");

        // Delete
        repo.delete_provider(&provider.id).await.unwrap();
        let deleted = repo.get_provider_by_id(&provider.id).await.unwrap();
        assert!(deleted.is_none());
    }
}
