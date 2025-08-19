// 供应商相关 IPC 命令

use crate::models::{
    ApiResponse, ListModelsRequest, ListModelsResponse, Model, ProbeResult, Provider,
    ProviderConfig, ToggleModelRequest, ToggleProviderRequest, UUID,
};
use crate::services::ProviderService;
use tauri::State;

/// 获取供应商列表
#[tauri::command]
pub async fn provider_list(
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Vec<Provider>>, String> {
    match provider_service.list_providers().await {
        Ok(providers) => Ok(ApiResponse::success(providers)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取供应商详情
#[tauri::command]
pub async fn provider_get(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Provider>, String> {
    match provider_service.get_provider(&provider_id).await {
        Ok(provider) => Ok(ApiResponse::success(provider)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 创建供应商
#[tauri::command]
pub async fn provider_create(
    config: ProviderConfig,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Provider>, String> {
    match provider_service.create_provider(config).await {
        Ok(provider) => Ok(ApiResponse::success(provider)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 更新供应商配置
#[tauri::command]
pub async fn provider_update(
    provider_id: UUID,
    config: ProviderConfig,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Provider>, String> {
    match provider_service.update_provider(&provider_id, config).await {
        Ok(provider) => Ok(ApiResponse::success(provider)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 删除供应商
#[tauri::command]
pub async fn provider_delete(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<()>, String> {
    match provider_service.delete_provider(&provider_id).await {
        Ok(_) => Ok(ApiResponse::success(())),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 探活检测供应商
#[tauri::command]
pub async fn provider_probe(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<ProbeResult>, String> {
    match provider_service.probe_provider(&provider_id).await {
        Ok(result) => Ok(ApiResponse::success(result)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取供应商模型列表
#[tauri::command]
pub async fn provider_list_models(
    request: ListModelsRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<ListModelsResponse>, String> {
    let force_refresh = request.force_refresh.unwrap_or(false);

    match provider_service
        .get_provider_models(&request.provider_id, force_refresh)
        .await
    {
        Ok(models) => {
            let response = ListModelsResponse {
                models,
                cached: !force_refresh,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
            };
            Ok(ApiResponse::success(response))
        }
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 切换供应商启用状态
#[tauri::command]
pub async fn provider_toggle(
    request: ToggleProviderRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Provider>, String> {
    match provider_service
        .toggle_provider(&request.provider_id, request.enabled)
        .await
    {
        Ok(provider) => Ok(ApiResponse::success(provider)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 切换模型启用状态
#[tauri::command]
pub async fn provider_toggle_model(
    request: ToggleModelRequest,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<()>, String> {
    match provider_service
        .toggle_model(&request.provider_id, &request.model_id, request.enabled)
        .await
    {
        Ok(_) => Ok(ApiResponse::success(())),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取所有可用模型列表
#[tauri::command]
pub async fn provider_get_available_models(
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Vec<(Provider, Vec<Model>)>>, String> {
    match provider_service.list_providers().await {
        Ok(providers) => {
            let mut result = vec![];

            for provider in providers {
                if provider.enabled {
                    match provider_service
                        .get_provider_models(&provider.id, false)
                        .await
                    {
                        Ok(models) => {
                            let enabled_models: Vec<Model> =
                                models.into_iter().filter(|m| m.enabled).collect();
                            if !enabled_models.is_empty() {
                                result.push((provider, enabled_models));
                            }
                        }
                        Err(_) => continue, // 忽略获取失败的供应商
                    }
                }
            }

            Ok(ApiResponse::success(result))
        }
        Err(error) => Ok(ApiResponse::error(error)),
    }
}
