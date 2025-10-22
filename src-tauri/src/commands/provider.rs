// 供应商相关 IPC 命令

use crate::models::{
    AddProviderRequest, AppError, ListModelsRequest, ListModelsResponse,
    ToggleModelFavoriteRequest, ToggleModelRequest, ToggleProviderRequest,
};
use crate::services::ProviderService;
use crate::storage::types::{Model, Provider, ProviderWithModels, UUID};
use tauri::State;

/// 获取供应商列表
#[tauri::command]
pub async fn provider_list(
    provider_service: State<'_, ProviderService>,
) -> Result<Vec<Provider>, AppError> {
    provider_service.list_providers().await
}

/// 获取供应商详情
#[tauri::command]
pub async fn provider_get(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<Provider, AppError> {
    provider_service.get_provider(&provider_id).await
}

/// 获取带模型的供应商详情
#[tauri::command]
pub async fn provider_get_with_models(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<ProviderWithModels, AppError> {
    provider_service
        .get_provider_with_models(&provider_id)
        .await
}

/// 创建供应商
#[tauri::command]
pub async fn provider_create(
    config: AddProviderRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<Provider, AppError> {
    provider_service.create_provider(config).await
}

/// 更新供应商配置
#[tauri::command]
pub async fn provider_update(
    provider_id: UUID,
    config: AddProviderRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<Provider, AppError> {
    println!("provider_update: {:?}", config);
    println!("provider_id: {:?}", provider_id);
    provider_service.update_provider(&provider_id, config).await
}

/// 删除供应商
#[tauri::command]
pub async fn provider_delete(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<(), AppError> {
    provider_service.delete_provider(&provider_id).await
}

/// 获取供应商模型列表
#[tauri::command]
pub async fn provider_list_models(
    request: ListModelsRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<ListModelsResponse, AppError> {
    let force_refresh = request.force_refresh.unwrap_or(false);

    let models = provider_service
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

/// 切换供应商启用状态
#[tauri::command]
pub async fn provider_toggle(
    request: ToggleProviderRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<Provider, AppError> {
    provider_service
        .toggle_provider(&request.provider_id, request.enabled)
        .await
}

/// 切换模型启用状态
#[tauri::command]
pub async fn provider_toggle_model(
    request: ToggleModelRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<(), AppError> {
    provider_service
        .toggle_model(&request.provider_id, &request.model_id, request.enabled)
        .await
}

/// 切换模型收藏状态
#[tauri::command]
pub async fn provider_toggle_model_favorite(
    request: ToggleModelFavoriteRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<(), AppError> {
    provider_service
        .toggle_favorite_model(&request.provider_id, &request.model_id, request.favorite)
        .await
}

/// 获取所有供应商及其模型（包含收藏状态）
#[tauri::command]
pub async fn provider_get_all_with_models(
    force_refresh: Option<bool>,
    provider_service: State<'_, ProviderService>,
) -> Result<Vec<ProviderWithModels>, AppError> {
    let force_refresh = force_refresh.unwrap_or(false);
    let providers = provider_service.list_providers().await?;
    let mut result = Vec::new();

    for provider in providers {
        match provider_service
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
pub async fn provider_get_favorite_models(
    provider_service: State<'_, ProviderService>,
) -> Result<Vec<Model>, AppError> {
    let providers = provider_service.list_providers().await?;
    let mut favorite_models = Vec::new();

    for provider in providers {
        match provider_service
            .get_provider_models(&provider.id, false)
            .await
        {
            Ok(models) => {
                favorite_models.extend(models.into_iter().filter(|m| m.favorite));
            }
            Err(_) => continue, // 忽略获取失败的供应商
        }
    }

    Ok(favorite_models)
}

/// 统计使用指定供应商的聊天数量
#[tauri::command]
pub async fn provider_count_chats(
    provider_id: String,
    provider_service: State<'_, ProviderService>,
) -> Result<i32, AppError> {
    provider_service
        .count_chats_using_provider(&provider_id)
        .await
}
