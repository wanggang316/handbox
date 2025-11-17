use crate::models::{AppError, SearchRequest, SearchResponse};
use crate::services::SearchService;
use tauri::State;

#[tauri::command]
pub async fn search_query(
    request: SearchRequest,
    search_service: State<'_, SearchService>,
) -> Result<SearchResponse, AppError> {
    search_service.search(request).await
}

#[tauri::command]
pub async fn search_history(
    limit: Option<usize>,
    search_service: State<'_, SearchService>,
) -> Result<Vec<String>, AppError> {
    search_service.get_history(limit).await
}

#[tauri::command]
pub async fn search_add_history(
    query: String,
    search_service: State<'_, SearchService>,
) -> Result<(), AppError> {
    search_service.add_history_entry(&query).await
}

#[tauri::command]
pub async fn search_clear_history(
    search_service: State<'_, SearchService>,
) -> Result<(), AppError> {
    search_service.clear_history().await
}

#[tauri::command]
pub async fn search_suggestions(
    query: String,
    limit: Option<usize>,
    search_service: State<'_, SearchService>,
) -> Result<Vec<String>, AppError> {
    search_service.get_suggestions(&query, limit).await
}
