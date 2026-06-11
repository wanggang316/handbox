// 模型相关 IPC 命令

use crate::models::{
    AppError, ListModelsRequest, ModelResponse, ToggleModelFavoriteRequest, ToggleModelRequest,
};
use crate::services::ModelService;
use tauri::State;

/// 获取供应商模型列表
#[tauri::command]
pub async fn model_list_by_provider(
    request: ListModelsRequest,
    model_service: State<'_, ModelService>,
) -> Result<Vec<ModelResponse>, AppError> {
    let refresh_from_remote = request.refresh_from_remote.unwrap_or(false);

    model_service
        .get_provider_models(&request.provider_id, refresh_from_remote)
        .await
}

/// 切换模型启用状态
#[tauri::command]
pub async fn model_toggle(
    request: ToggleModelRequest,
    model_service: State<'_, ModelService>,
) -> Result<(), AppError> {
    model_service
        .toggle_model(&request.provider_id, &request.model_id, request.enabled)
        .await
}

/// 切换模型收藏状态
#[tauri::command]
pub async fn model_toggle_favorite(
    request: ToggleModelFavoriteRequest,
    model_service: State<'_, ModelService>,
) -> Result<(), AppError> {
    model_service
        .toggle_favorite_model(&request.provider_id, &request.model_id, request.favorite)
        .await
}

/// 统计使用指定模型的聊天数量
#[tauri::command]
pub async fn model_count_chats(
    model_id: String,
    model_service: State<'_, ModelService>,
) -> Result<i32, AppError> {
    model_service.count_chats_using_model(&model_id).await
}

/// 为自定义供应商手动添加模型（自定义端点的模型不在 hand-ai 目录中）。
#[tauri::command]
pub async fn model_add(
    provider_id: String,
    model_id: String,
    name: Option<String>,
    model_service: State<'_, ModelService>,
) -> Result<ModelResponse, AppError> {
    model_service
        .add_manual_model(&provider_id, &model_id, name)
        .await
}
