// 单词相关 IPC 命令

use crate::models::{
    AppError, CreateWordLookupRequest, CreateWordRequest, ListWordLookupHistoryRequest,
    ListWordsRequest, ReviewWordRequest, TranslateWordRequest, TranslateWordResponse,
    UpdateWordRequest, WordDetail,
};
use crate::services::WordService;
use crate::storage::types::Word;
use tauri::State;

#[tauri::command]
pub async fn word_create(
    request: CreateWordRequest,
    word_service: State<'_, WordService>,
) -> Result<Word, AppError> {
    word_service.create_word(request).await
}

#[tauri::command]
pub async fn word_list(
    request: Option<ListWordsRequest>,
    word_service: State<'_, WordService>,
) -> Result<Vec<Word>, AppError> {
    let request = request.unwrap_or(ListWordsRequest {
        query: None,
        tag: None,
        limit: Some(50),
        offset: Some(0),
    });

    let limit = request.limit.unwrap_or(50);
    let offset = request.offset.unwrap_or(0);

    word_service
        .list_words(request.query, request.tag, limit, offset)
        .await
}

#[tauri::command]
pub async fn word_get(
    word_id: String,
    word_service: State<'_, WordService>,
) -> Result<WordDetail, AppError> {
    word_service.get_word_detail(&word_id).await
}

#[tauri::command]
pub async fn word_update(
    request: UpdateWordRequest,
    word_service: State<'_, WordService>,
) -> Result<Word, AppError> {
    word_service.update_word(request).await
}

#[tauri::command]
pub async fn word_delete(
    word_id: String,
    word_service: State<'_, WordService>,
) -> Result<(), AppError> {
    word_service.delete_word(&word_id).await
}

#[tauri::command]
pub async fn word_review(
    request: ReviewWordRequest,
    word_service: State<'_, WordService>,
) -> Result<crate::storage::types::WordReview, AppError> {
    word_service.review_word(request).await
}

#[tauri::command]
pub async fn word_translate(
    request: TranslateWordRequest,
    word_service: State<'_, WordService>,
) -> Result<TranslateWordResponse, AppError> {
    word_service.translate_word(request).await
}

#[tauri::command]
pub async fn word_lookup_record(
    request: CreateWordLookupRequest,
    word_service: State<'_, WordService>,
) -> Result<crate::storage::types::WordLookupHistory, AppError> {
    word_service.record_lookup_history(request).await
}

#[tauri::command]
pub async fn word_lookup_history(
    request: Option<ListWordLookupHistoryRequest>,
    word_service: State<'_, WordService>,
) -> Result<Vec<crate::storage::types::WordLookupHistory>, AppError> {
    let request = request.unwrap_or(ListWordLookupHistoryRequest {
        limit: Some(20),
        offset: Some(0),
    });
    word_service.list_lookup_history(request).await
}

#[tauri::command]
pub async fn word_lookup_delete(
    history_id: String,
    word_service: State<'_, WordService>,
) -> Result<(), AppError> {
    word_service.delete_lookup_history(&history_id).await
}
