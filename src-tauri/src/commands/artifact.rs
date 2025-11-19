// Artifact IPC 命令

use crate::models::AppError;
use crate::services::ArtifactService;
use crate::storage::types::{
    Artifact, ArtifactFilter, CreateArtifactRequest, ExecuteArtifactRequest, ExecutionResult,
    InstallArtifactRequest, UpdateArtifactRequest,
};
use tauri::State;

/// 创建 artifact
#[tauri::command]
pub async fn artifact_create(
    request: CreateArtifactRequest,
    service: State<'_, ArtifactService>,
) -> Result<Artifact, AppError> {
    tracing::info!("Creating artifact: {}", request.name);
    service.create_artifact(request).await
}

/// 更新 artifact
#[tauri::command]
pub async fn artifact_update(
    request: UpdateArtifactRequest,
    service: State<'_, ArtifactService>,
) -> Result<Artifact, AppError> {
    tracing::info!("Updating artifact: {}", request.id);
    service.update_artifact(request).await
}

/// 获取 artifact
#[tauri::command]
pub async fn artifact_get(
    artifact_id: String,
    service: State<'_, ArtifactService>,
) -> Result<Artifact, AppError> {
    tracing::info!("Getting artifact: {}", artifact_id);
    service.get_artifact(&artifact_id).await
}

/// 列出 artifacts
#[tauri::command]
pub async fn artifact_list(
    filter: Option<ArtifactFilter>,
    service: State<'_, ArtifactService>,
) -> Result<Vec<Artifact>, AppError> {
    tracing::info!("Listing artifacts with filter: {:?}", filter);
    let filter = filter.unwrap_or_else(|| ArtifactFilter {
        search: None,
        artifact_type: None,
        is_builtin: None,
        is_installed: None,
        tags: None,
        sort_by: Some("updated_at".to_string()),
        sort_order: Some("DESC".to_string()),
        limit: Some(100),
        offset: Some(0),
    });
    service.list_artifacts(filter).await
}

/// 删除 artifact
#[tauri::command]
pub async fn artifact_delete(
    artifact_id: String,
    service: State<'_, ArtifactService>,
) -> Result<(), AppError> {
    tracing::info!("Deleting artifact: {}", artifact_id);
    service.delete_artifact(&artifact_id).await
}

/// 安装 artifact
#[tauri::command]
pub async fn artifact_install(
    request: InstallArtifactRequest,
    service: State<'_, ArtifactService>,
) -> Result<Artifact, AppError> {
    tracing::info!("Installing artifact: {}", request.artifact_id);
    service.install_artifact(request).await
}

/// 执行 artifact
#[tauri::command]
pub async fn artifact_execute(
    request: ExecuteArtifactRequest,
    service: State<'_, ArtifactService>,
) -> Result<ExecutionResult, AppError> {
    tracing::info!("Executing artifact: {}", request.artifact_id);
    service.execute_artifact(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ArtifactService;
    use crate::storage::types::{ArtifactType, ExecutionConfig};
    use crate::storage::{ArtifactRepository, Database};
    use std::sync::Arc;
    use tauri::test::{mock_builder, MockRuntime};
    use tempfile::tempdir;

    async fn create_test_service() -> (ArtifactService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.unwrap());
        let repo = Arc::new(ArtifactRepository::new(db));

        let app = mock_builder().build();
        let app_handle = app.handle();

        let service = ArtifactService::new(repo, app_handle.clone());
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_list_artifacts() {
        let (service, _temp_dir) = create_test_service().await;

        let request = CreateArtifactRequest {
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
            execution_config: Some(ExecutionConfig::default()),
            tags: Some(vec!["test".to_string()]),
            icon: Some("🐚".to_string()),
        };

        let artifact = service.create_artifact(request).await.unwrap();
        assert_eq!(artifact.name, "Test Shell App");

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

        let artifacts = service.list_artifacts(filter).await.unwrap();
        assert_eq!(artifacts.len(), 1);
    }
}
