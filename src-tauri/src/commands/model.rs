// 模型相关 IPC 命令

use crate::models::{
    AppError, ListModelsRequest, ListModelsResponse, ToggleModelFavoriteRequest, ToggleModelRequest,
};
use crate::services::{ModelService, ProviderService};
use crate::storage::types::{Model, ProviderWithModels};
use tauri::State;

/// 获取供应商模型列表
#[tauri::command]
pub async fn model_list_by_provider(
    request: ListModelsRequest,
    model_service: State<'_, ModelService>,
) -> Result<ListModelsResponse, AppError> {
    let force_refresh = request.force_refresh.unwrap_or(false);

    let models = model_service
        .get_provider_models(&request.provider_id, force_refresh)
        .await?;

    Ok(ListModelsResponse {
        models,
        cached: !force_refresh,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    })
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

/// 获取所有供应商及其模型（包含收藏状态）
#[tauri::command]
pub async fn model_get_all_with_providers(
    force_refresh: Option<bool>,
    provider_service: State<'_, ProviderService>,
    model_service: State<'_, ModelService>,
) -> Result<Vec<ProviderWithModels>, AppError> {
    let force_refresh = force_refresh.unwrap_or(false);
    let providers = provider_service.list_providers().await?;
    let mut result = Vec::new();

    for provider in providers {
        match model_service
            .get_provider_models(&provider.id, force_refresh)
            .await
        {
            Ok(models) => {
                result.push(ProviderWithModels {
                    id: provider.id,
                    name: provider.name,
                    provider_type: provider.provider_type,
                    base_url: provider.base_url,
                    api_key: provider.api_key,
                    enabled: provider.enabled,
                    created_at: provider.created_at,
                    updated_at: provider.updated_at,
                    models,
                });
            }
            Err(_) => {
                // 即使获取模型失败，也返回供应商信息（空模型列表）
                result.push(ProviderWithModels {
                    id: provider.id,
                    name: provider.name,
                    provider_type: provider.provider_type,
                    base_url: provider.base_url,
                    api_key: provider.api_key,
                    enabled: provider.enabled,
                    created_at: provider.created_at,
                    updated_at: provider.updated_at,
                    models: Vec::new(),
                });
            }
        }
    }

    Ok(result)
}

/// 获取所有收藏的模型
#[tauri::command]
pub async fn model_get_favorites(
    model_service: State<'_, ModelService>,
) -> Result<Vec<Model>, AppError> {
    model_service.get_favorite_models().await
}

/// 获取所有可用模型（所有启用供应商的启用模型）
#[tauri::command]
pub async fn model_get_available(
    model_service: State<'_, ModelService>,
) -> Result<Vec<Model>, AppError> {
    model_service.get_available_models().await
}

/// 统计使用指定模型的聊天数量
#[tauri::command]
pub async fn model_count_chats(
    model_id: String,
    model_service: State<'_, ModelService>,
) -> Result<i32, AppError> {
    model_service.count_chats_using_model(&model_id).await
}
