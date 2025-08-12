// Artifact 相关 IPC 命令

use crate::models::{ApiResponse, AppError, Artifact, CreateArtifactRequest, UUID};
use crate::services::ArtifactService;
use tauri::State;

/// 获取 Artifact 列表
#[tauri::command]
pub async fn artifact_list(
    artifact_service: State<'_, ArtifactService>,
) -> Result<ApiResponse<Vec<Artifact>>, String> {
    // TODO: 实现 Artifact 列表获取
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}

/// 创建 Artifact
#[tauri::command]
pub async fn artifact_create(
    request: CreateArtifactRequest,
    artifact_service: State<'_, ArtifactService>,
) -> Result<ApiResponse<Artifact>, String> {
    // TODO: 实现 Artifact 创建
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}
