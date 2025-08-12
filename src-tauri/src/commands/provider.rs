// 供应商相关 IPC 命令

use crate::models::{ApiResponse, AppError, ProbeResult, Provider, ProviderConfig, UUID};
use crate::services::ProviderService;
use tauri::State;

/// 获取供应商列表
#[tauri::command]
pub async fn provider_list(
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Vec<Provider>>, String> {
    // TODO: 实现供应商列表获取
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}

/// 创建供应商
#[tauri::command]
pub async fn provider_create(
    config: ProviderConfig,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<Provider>, String> {
    // TODO: 实现供应商创建
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}

/// 探活检测供应商
#[tauri::command]
pub async fn provider_probe(
    provider_id: UUID,
    provider_service: State<'_, ProviderService>,
) -> Result<ApiResponse<ProbeResult>, String> {
    // TODO: 实现供应商探活
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}
