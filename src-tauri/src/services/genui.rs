// GenUI 服务实现

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{GenUi, UUID};
use crate::storage::GenUiRepository;
use std::sync::Arc;

/// GenUI 服务
#[derive(Clone)]
pub struct GenUiService {
    repository: GenUiRepository,
}

impl GenUiService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: GenUiRepository::new(db),
        }
    }

    /// 创建 GenUI
    pub async fn create_genui(&self, name: String, spec: String) -> Result<GenUi, AppError> {
        if name.trim().is_empty() {
            return Err(AppError::validation_error("GenUI name must not be empty"));
        }

        let now = Self::current_timestamp();
        let genui = GenUi {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            spec,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_genui(&genui).await?;
        Ok(genui)
    }

    /// 获取 GenUI 列表
    pub async fn list_genui(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<GenUi>, AppError> {
        self.repository
            .list_genui(limit.unwrap_or(100), offset.unwrap_or(0))
            .await
    }

    /// 获取 GenUI 详情
    pub async fn get_genui(&self, id: UUID) -> Result<GenUi, AppError> {
        match self.repository.get_genui_by_id(&id).await? {
            Some(genui) => Ok(genui),
            None => Err(AppError::not_found(&format!("GenUI not found: {}", id))),
        }
    }

    /// 更新 GenUI（名称 / spec 按需更新）
    pub async fn update_genui(
        &self,
        id: UUID,
        name: Option<String>,
        spec: Option<String>,
    ) -> Result<GenUi, AppError> {
        let mut genui = self.get_genui(id).await?;

        if let Some(name) = name {
            if name.trim().is_empty() {
                return Err(AppError::validation_error("GenUI name must not be empty"));
            }
            genui.name = name;
        }
        if let Some(spec) = spec {
            genui.spec = spec;
        }

        genui.updated_at = Self::current_timestamp();
        self.repository.update_genui(&genui).await?;
        Ok(genui)
    }

    /// 删除 GenUI
    pub async fn delete_genui(&self, id: UUID) -> Result<(), AppError> {
        self.get_genui(id.clone()).await?;
        self.repository.delete_genui(&id).await
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        )
    }

    #[tokio::test]
    async fn creates_and_fetches_genui() {
        let service = GenUiService::new(create_test_database().await);

        let created = service
            .create_genui(
                "My Card".to_string(),
                r#"{"root":"x","elements":{}}"#.to_string(),
            )
            .await
            .expect("create failed");

        assert_eq!(created.name, "My Card");

        let fetched = service.get_genui(created.id.clone()).await.unwrap();
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.spec, r#"{"root":"x","elements":{}}"#);
    }

    #[tokio::test]
    async fn rejects_empty_name_on_create() {
        let service = GenUiService::new(create_test_database().await);

        let err = service
            .create_genui("   ".to_string(), "{}".to_string())
            .await
            .expect_err("expected validation error");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn lists_genui_sorted_by_updated_at() {
        let service = GenUiService::new(create_test_database().await);

        service
            .create_genui("First".to_string(), "{}".to_string())
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        service
            .create_genui("Second".to_string(), "{}".to_string())
            .await
            .unwrap();

        let items = service.list_genui(Some(10), Some(0)).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "Second");
        assert_eq!(items[1].name, "First");
    }

    #[tokio::test]
    async fn updates_name_and_spec() {
        let service = GenUiService::new(create_test_database().await);

        let created = service
            .create_genui("Old".to_string(), "{}".to_string())
            .await
            .unwrap();

        let updated = service
            .update_genui(
                created.id.clone(),
                Some("New".to_string()),
                Some(r#"{"root":"y","elements":{}}"#.to_string()),
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "New");
        assert_eq!(updated.spec, r#"{"root":"y","elements":{}}"#);
    }

    #[tokio::test]
    async fn get_missing_genui_returns_not_found() {
        let service = GenUiService::new(create_test_database().await);
        let err = service
            .get_genui("nope".to_string())
            .await
            .expect_err("expected error");
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn delete_removes_genui() {
        let service = GenUiService::new(create_test_database().await);
        let created = service
            .create_genui("Doomed".to_string(), "{}".to_string())
            .await
            .unwrap();

        service.delete_genui(created.id.clone()).await.unwrap();

        let err = service
            .get_genui(created.id)
            .await
            .expect_err("expected missing genui");
        assert_eq!(err.code, "NOT_FOUND");
    }
}
