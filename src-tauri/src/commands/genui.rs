// GenUI 相关 IPC 命令

use crate::models::AppError;
use crate::services::GenUiService;
use crate::storage::types::{CreateGenUiRequest, GenUi, UpdateGenUiRequest, UUID};
use tauri::State;

/// 创建新的 GenUI
#[tauri::command]
pub async fn genui_create(
    request: CreateGenUiRequest,
    genui_service: State<'_, GenUiService>,
) -> Result<GenUi, AppError> {
    genui_service.create_genui(request.name, request.spec).await
}

/// 获取 GenUI 列表
#[tauri::command]
pub async fn genui_list(
    limit: Option<i32>,
    offset: Option<i32>,
    genui_service: State<'_, GenUiService>,
) -> Result<Vec<GenUi>, AppError> {
    genui_service.list_genui(limit, offset).await
}

/// 获取 GenUI 详情
#[tauri::command]
pub async fn genui_get(
    genui_id: UUID,
    genui_service: State<'_, GenUiService>,
) -> Result<GenUi, AppError> {
    genui_service.get_genui(genui_id).await
}

/// 更新 GenUI（名称 / spec）
#[tauri::command]
pub async fn genui_update(
    genui_id: UUID,
    request: UpdateGenUiRequest,
    genui_service: State<'_, GenUiService>,
) -> Result<GenUi, AppError> {
    genui_service
        .update_genui(genui_id, request.name, request.spec)
        .await
}

/// 删除 GenUI
#[tauri::command]
pub async fn genui_delete(
    genui_id: UUID,
    genui_service: State<'_, GenUiService>,
) -> Result<(), AppError> {
    genui_service.delete_genui(genui_id).await
}
