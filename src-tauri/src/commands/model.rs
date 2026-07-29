use crate::models::{
    AppError, ListModelsRequest, ModelResponse, ToggleModelFavoriteRequest, ToggleModelRequest,
};
use crate::services::ModelService;
use tauri::State;

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

#[tauri::command]
pub async fn model_toggle(
    request: ToggleModelRequest,
    model_service: State<'_, ModelService>,
) -> Result<(), AppError> {
    model_service
        .toggle_model(&request.provider_id, &request.model_id, request.enabled)
        .await
}

#[tauri::command]
pub async fn model_toggle_favorite(
    request: ToggleModelFavoriteRequest,
    model_service: State<'_, ModelService>,
) -> Result<(), AppError> {
    model_service
        .toggle_favorite_model(&request.provider_id, &request.model_id, request.favorite)
        .await
}

/// Manually adds a model for a custom provider (custom-endpoint models are not
/// in the hand-ai catalog).
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
