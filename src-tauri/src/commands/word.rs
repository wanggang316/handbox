// 单词相关 IPC 命令

use crate::models::{
    AppError, CreateWordRequest, ListWordsRequest, UpdateWordRequest,
};
use crate::services::{MessageService, WordService};
use crate::storage::types::{Message, Word};
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
) -> Result<Word, AppError> {
    word_service.get_word(&word_id).await
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
pub async fn word_translation_history(
    session_id: String,
    limit: Option<i32>,
    offset: Option<i32>,
    message_service: State<'_, MessageService>,
) -> Result<Vec<Message>, AppError> {
    let limit = limit.unwrap_or(20);
    let offset = offset.unwrap_or(0);

    message_service
        .get_messages(session_id, Some(limit), Some(offset))
        .await
}
