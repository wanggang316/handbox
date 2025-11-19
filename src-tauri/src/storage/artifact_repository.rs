// Artifact 数据访问层

use crate::models::AppError;
use crate::storage::types::{Artifact, ArtifactFilter, ArtifactType, ModelParameters, ExecutionConfig, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Artifact 仓储层
#[derive(Clone)]
pub struct ArtifactRepository {
    db: Arc<Database>,
}

impl ArtifactRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建 Artifact
    pub async fn create_artifact(&self, artifact: &Artifact) -> Result<(), AppError> {
        let artifact_type = artifact.artifact_type.to_string();
        let model_parameters_json = artifact
            .model_parameters
            .as_ref()
            .and_then(|p| serde_json::to_string(p).ok());
        let tools_json = serde_json::to_string(&artifact.tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid tools: {}", e)))?;
        let execution_config_json = serde_json::to_string(&artifact.execution_config)
            .map_err(|e| AppError::validation_error(&format!("Invalid execution config: {}", e)))?;
        let tags_json = serde_json::to_string(&artifact.tags)
            .map_err(|e| AppError::validation_error(&format!("Invalid tags: {}", e)))?;

        let query = r#"
            INSERT INTO artifacts (
                id, name, description, type, entry_file, source_path,
                model_id, provider_id, system_prompt, model_parameters, tools,
                execution_config, is_builtin, is_installed, installed_version,
                installed_at, last_run_at, run_count, tags, icon, author,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                $15, $16, $17, $18, $19, $20, $21, $22, $23
            )
        "#;

        sqlx::query(query)
            .bind(&artifact.id)
            .bind(&artifact.name)
            .bind(&artifact.description)
            .bind(&artifact_type)
            .bind(&artifact.entry_file)
            .bind(&artifact.source_path)
            .bind(&artifact.model_id)
            .bind(&artifact.provider_id)
            .bind(&artifact.system_prompt)
            .bind(model_parameters_json)
            .bind(&tools_json)
            .bind(&execution_config_json)
            .bind(artifact.is_builtin)
            .bind(artifact.is_installed)
            .bind(&artifact.installed_version)
            .bind(artifact.installed_at)
            .bind(artifact.last_run_at)
            .bind(artifact.run_count)
            .bind(&tags_json)
            .bind(&artifact.icon)
            .bind(&artifact.author)
            .bind(artifact.created_at)
            .bind(artifact.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create artifact: {}", e)))?;

        Ok(())
    }

    /// 更新 Artifact
    pub async fn update_artifact(&self, artifact: &Artifact) -> Result<(), AppError> {
        let artifact_type = artifact.artifact_type.to_string();
        let model_parameters_json = artifact
            .model_parameters
            .as_ref()
            .and_then(|p| serde_json::to_string(p).ok());
        let tools_json = serde_json::to_string(&artifact.tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid tools: {}", e)))?;
        let execution_config_json = serde_json::to_string(&artifact.execution_config)
            .map_err(|e| AppError::validation_error(&format!("Invalid execution config: {}", e)))?;
        let tags_json = serde_json::to_string(&artifact.tags)
            .map_err(|e| AppError::validation_error(&format!("Invalid tags: {}", e)))?;

        let query = r#"
            UPDATE artifacts SET
                name = $2, description = $3, type = $4, entry_file = $5, source_path = $6,
                model_id = $7, provider_id = $8, system_prompt = $9, model_parameters = $10,
                tools = $11, execution_config = $12, is_installed = $13, installed_version = $14,
                installed_at = $15, last_run_at = $16, run_count = $17, tags = $18,
                icon = $19, author = $20, updated_at = $21
            WHERE id = $1
        "#;

        let result = sqlx::query(query)
            .bind(&artifact.id)
            .bind(&artifact.name)
            .bind(&artifact.description)
            .bind(&artifact_type)
            .bind(&artifact.entry_file)
            .bind(&artifact.source_path)
            .bind(&artifact.model_id)
            .bind(&artifact.provider_id)
            .bind(&artifact.system_prompt)
            .bind(model_parameters_json)
            .bind(&tools_json)
            .bind(&execution_config_json)
            .bind(artifact.is_installed)
            .bind(&artifact.installed_version)
            .bind(artifact.installed_at)
            .bind(artifact.last_run_at)
            .bind(artifact.run_count)
            .bind(&tags_json)
            .bind(&artifact.icon)
            .bind(&artifact.author)
            .bind(artifact.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update artifact: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Artifact not found"));
        }

        Ok(())
    }

    /// 根据 ID 获取 Artifact
    pub async fn get_artifact_by_id(&self, id: &UUID) -> Result<Option<Artifact>, AppError> {
        let query = r#"
            SELECT * FROM artifacts WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get artifact: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_artifact(row)?))
        } else {
            Ok(None)
        }
    }

    /// 获取 Artifact 列表
    pub async fn list_artifacts(&self, filter: &ArtifactFilter) -> Result<Vec<Artifact>, AppError> {
        let mut query = String::from("SELECT * FROM artifacts WHERE 1=1");
        let mut conditions = Vec::new();

        if let Some(search) = &filter.search {
            conditions.push(format!(
                " AND (name LIKE '%{}%' OR description LIKE '%{}%')",
                search, search
            ));
        }

        if let Some(artifact_type) = &filter.artifact_type {
            conditions.push(format!(" AND type = '{}'", artifact_type));
        }

        if let Some(is_builtin) = filter.is_builtin {
            conditions.push(format!(" AND is_builtin = {}", is_builtin as i32));
        }

        if let Some(is_installed) = filter.is_installed {
            conditions.push(format!(" AND is_installed = {}", is_installed as i32));
        }

        for condition in conditions {
            query.push_str(&condition);
        }

        // Sort
        let sort_by = filter.sort_by.as_deref().unwrap_or("updated_at");
        let sort_order = filter.sort_order.as_deref().unwrap_or("DESC");
        query.push_str(&format!(" ORDER BY {} {}", sort_by, sort_order));

        // Pagination
        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let rows = sqlx::query(&query)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list artifacts: {}", e)))?;

        let mut artifacts = Vec::new();
        for row in rows {
            artifacts.push(self.row_to_artifact(row)?);
        }

        Ok(artifacts)
    }

    /// 删除 Artifact
    pub async fn delete_artifact(&self, id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM artifacts WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete artifact: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::validation_error("Artifact not found"));
        }

        Ok(())
    }

    /// 更新运行统计
    pub async fn update_run_stats(
        &self,
        id: &UUID,
        last_run_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE artifacts
            SET last_run_at = $2, run_count = run_count + 1, updated_at = $2
            WHERE id = $1
        "#;

        sqlx::query(query)
            .bind(id)
            .bind(last_run_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update run stats: {}", e)))?;

        Ok(())
    }

    /// 标记为已安装
    pub async fn mark_installed(
        &self,
        id: &UUID,
        version: &str,
        installed_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE artifacts
            SET is_installed = 1, installed_version = $2, installed_at = $3, updated_at = $3
            WHERE id = $1
        "#;

        sqlx::query(query)
            .bind(id)
            .bind(version)
            .bind(installed_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to mark as installed: {}", e)))?;

        Ok(())
    }

    // 辅助方法：将数据库行转换为 Artifact
    fn row_to_artifact(&self, row: sqlx::sqlite::SqliteRow) -> Result<Artifact, AppError> {
        let artifact_type_str: String = row.try_get("type")?;
        let artifact_type = artifact_type_str.parse::<ArtifactType>()
            .map_err(|e| AppError::internal_error(&e))?;

        let model_parameters: Option<ModelParameters> = row
            .try_get::<Option<String>, _>("model_parameters")?
            .and_then(|s| serde_json::from_str(&s).ok());

        let tools: Option<Vec<String>> = row
            .try_get::<Option<String>, _>("tools")?
            .and_then(|s| serde_json::from_str(&s).ok());

        let execution_config_str: String = row.try_get("execution_config")?;
        let execution_config: ExecutionConfig = serde_json::from_str(&execution_config_str)
            .unwrap_or_default();

        let tags: Vec<String> = row
            .try_get::<Option<String>, _>("tags")?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Ok(Artifact {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            artifact_type,
            entry_file: row.try_get("entry_file")?,
            source_path: row.try_get("source_path")?,
            model_id: row.try_get("model_id")?,
            provider_id: row.try_get("provider_id")?,
            system_prompt: row.try_get("system_prompt")?,
            model_parameters,
            tools,
            execution_config,
            is_builtin: row.try_get("is_builtin")?,
            is_installed: row.try_get("is_installed")?,
            installed_version: row.try_get("installed_version")?,
            installed_at: row.try_get("installed_at")?,
            last_run_at: row.try_get("last_run_at")?,
            run_count: row.try_get("run_count")?,
            tags,
            icon: row.try_get("icon")?,
            author: row.try_get("author")?,
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
    async fn test_artifact_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ArtifactRepository::new(Arc::new(db));

        let now = chrono::Utc::now().timestamp_millis();
        let artifact = Artifact {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Shell App".to_string(),
            description: Some("A test shell application".to_string()),
            artifact_type: ArtifactType::Shell,
            entry_file: "main.sh".to_string(),
            source_path: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            model_parameters: None,
            tools: None,
            execution_config: ExecutionConfig::default(),
            is_builtin: false,
            is_installed: false,
            installed_version: None,
            installed_at: None,
            last_run_at: None,
            run_count: 0,
            tags: vec!["test".to_string()],
            icon: Some("🐚".to_string()),
            author: Some("Test Author".to_string()),
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_artifact(&artifact).await.unwrap();

        // Get by ID
        let fetched = repo.get_artifact_by_id(&artifact.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.as_ref().unwrap().name, artifact.name);

        // List
        let filter = ArtifactFilter {
            search: None,
            artifact_type: Some(ArtifactType::Shell),
            is_builtin: None,
            is_installed: None,
            tags: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        };
        let artifacts = repo.list_artifacts(&filter).await.unwrap();
        assert_eq!(artifacts.len(), 1);

        // Update run stats
        let new_time = chrono::Utc::now().timestamp_millis();
        repo.update_run_stats(&artifact.id, new_time).await.unwrap();

        let updated = repo.get_artifact_by_id(&artifact.id).await.unwrap().unwrap();
        assert_eq!(updated.run_count, 1);

        // Mark as installed
        repo.mark_installed(&artifact.id, "1.0.0", new_time).await.unwrap();
        let installed = repo.get_artifact_by_id(&artifact.id).await.unwrap().unwrap();
        assert!(installed.is_installed);

        // Delete
        repo.delete_artifact(&artifact.id).await.unwrap();
        let deleted = repo.get_artifact_by_id(&artifact.id).await.unwrap();
        assert!(deleted.is_none());
    }
}
