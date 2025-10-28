// 供应商相关 IPC 命令

use crate::models::{AddProviderRequest, AppError, ProviderWithModels, ToggleProviderRequest};
use crate::services::{ModelService, ProviderService};
use crate::storage::types::{Provider, UUID};
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

/// 获取所有供应商及其模型列表
#[tauri::command]
pub async fn provider_list_with_models(
    refresh_from_remote: Option<bool>,
    provider_service: State<'_, ProviderService>,
    model_service: State<'_, ModelService>,
) -> Result<Vec<ProviderWithModels>, AppError> {
    let refresh_from_remote = refresh_from_remote.unwrap_or(false);
    let providers = provider_service.list_providers().await?;
    let mut result = Vec::new();

    for provider in providers {
        match model_service
            .get_provider_models(&provider.id, refresh_from_remote)
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
