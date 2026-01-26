// 收藏相关 IPC 命令

use crate::models::AppError;
use crate::storage::types::favorite::FavoriteTag;
use crate::storage::types::{CreateFavoriteRequest, Favorite, FavoriteMessageType, UUID};
use crate::storage::FavoriteRepository;
use serde::{Deserialize, Serialize};
use tauri::State;
use url::Url;

const EXTERNAL_CHAT_ID: &str = "external";

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteToggleRequest {
    #[serde(rename = "messageId")]
    pub message_id: UUID,
    #[serde(rename = "chatId")]
    pub chat_id: UUID,
    pub content: String,
    pub role: String,
    #[serde(rename = "messageType")]
    pub message_type: String,
    pub tags: Vec<FavoriteTag>,
    pub note: Option<String>,
    #[serde(rename = "context")]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteIsFavoritedRequest {
    #[serde(rename = "messageId")]
    pub message_id: UUID,
    #[serde(rename = "chatId")]
    pub chat_id: UUID,
    #[serde(rename = "messageType")]
    pub message_type: String,
}

#[tauri::command]
pub async fn favorite_toggle(
    request: FavoriteToggleRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!(
        "[favorite_toggle] IPC command called for message_id: {}",
        request.message_id
    );

    let message_type_enum = match request.message_type.as_str() {
        "text" => FavoriteMessageType::Text,
        "image" => FavoriteMessageType::Image,
        "chat" => FavoriteMessageType::Chat,
        "external" => FavoriteMessageType::External,
        _ => FavoriteMessageType::Message,
    };

    let create_request = CreateFavoriteRequest {
        message_id: request.message_id,
        chat_id: request.chat_id,
        content: request.content,
        role: request.role,
        message_type: message_type_enum,
        tags: request.tags,
        note: request.note,
        context: request.context,
        selection_text_raw: None,
        source_app_name: None,
        source_bundle_id: None,
        source_pid: None,
        source_app_path: None,
        source_app_version: None,
        source_window_title: None,
        source_url: None,
        source_domain: None,
        source_tab_title: None,
        selection_rect: None,
        capture_method: None,
        locale: None,
        input_language: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match favorite_repo.toggle_favorite(&create_request).await {
        Ok(is_favorited) => {
            tracing::info!(
                "[favorite_toggle] Command completed successfully, is_favorited: {}",
                is_favorited
            );
            Ok(is_favorited)
        }
        Err(e) => {
            tracing::error!("[favorite_toggle] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_is_favorited(
    request: FavoriteIsFavoritedRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!(
        "[favorite_is_favorited] IPC command called for message_id: {}",
        request.message_id
    );

    let message_type_enum = match request.message_type.as_str() {
        "text" => FavoriteMessageType::Text,
        "image" => FavoriteMessageType::Image,
        "chat" => FavoriteMessageType::Chat,
        "external" => FavoriteMessageType::External,
        _ => FavoriteMessageType::Message,
    };

    match favorite_repo
        .is_favorited(&request.message_id, &request.chat_id, &message_type_enum)
        .await
    {
        Ok(is_favorited) => {
            tracing::info!(
                "[favorite_is_favorited] Command completed, result: {}",
                is_favorited
            );
            Ok(is_favorited)
        }
        Err(e) => {
            tracing::error!("[favorite_is_favorited] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_list(
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<Vec<Favorite>, AppError> {
    tracing::info!("[favorite_list] IPC command called");

    match favorite_repo.get_all_favorites().await {
        Ok(favorites) => {
            tracing::info!(
                "[favorite_list] Command completed, returned {} favorites",
                favorites.len()
            );
            Ok(favorites)
        }
        Err(e) => {
            tracing::error!("[favorite_list] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteListByChatRequest {
    #[serde(rename = "chatId")]
    pub chat_id: UUID,
}

#[tauri::command]
pub async fn favorite_list_by_chat(
    request: FavoriteListByChatRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<Vec<Favorite>, AppError> {
    tracing::info!(
        "[favorite_list_by_chat] IPC command called for chat_id: {}",
        request.chat_id
    );

    match favorite_repo.get_favorites_by_chat(&request.chat_id).await {
        Ok(favorites) => {
            tracing::info!(
                "[favorite_list_by_chat] Command completed, returned {} favorites",
                favorites.len()
            );
            Ok(favorites)
        }
        Err(e) => {
            tracing::error!("[favorite_list_by_chat] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_list_tags(
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<Vec<FavoriteTag>, AppError> {
    tracing::info!("[favorite_list_tags] IPC command called");

    match favorite_repo.list_tags().await {
        Ok(tags) => {
            tracing::info!(
                "[favorite_list_tags] Command completed, returned {} tags",
                tags.len()
            );
            Ok(tags)
        }
        Err(e) => {
            tracing::error!("[favorite_list_tags] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteAddTagRequest {
    #[serde(rename = "favoriteId")]
    pub favorite_id: UUID,
    pub tag: FavoriteTag,
}

#[tauri::command]
pub async fn favorite_add_tag(
    request: FavoriteAddTagRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!(
        "[favorite_add_tag] IPC command called for favorite_id: {}",
        request.favorite_id
    );

    match favorite_repo
        .add_tag(&request.favorite_id, &request.tag)
        .await
    {
        Ok(()) => {
            tracing::info!("[favorite_add_tag] Command completed successfully");
            Ok(())
        }
        Err(e) => {
            tracing::error!("[favorite_add_tag] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteRemoveTagRequest {
    #[serde(rename = "favoriteId")]
    pub favorite_id: UUID,
    #[serde(rename = "tagName", alias = "tag_name")]
    pub tag_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextRange {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteSaveTextRangesRequest {
    #[serde(rename = "messageId")]
    pub message_id: UUID,
    #[serde(rename = "chatId")]
    pub chat_id: UUID,
    pub ranges: Vec<TextRange>,
    pub role: String,
    #[serde(default)]
    pub tags: Vec<FavoriteTag>,
    pub note: Option<String>,
    #[serde(rename = "context")]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteCreateExternalRequest {
    pub content: String,
    pub role: String,
    #[serde(default)]
    pub tags: Vec<FavoriteTag>,
    pub note: Option<String>,
    pub context: Option<String>,
    pub selection_text_raw: Option<String>,
    pub source_app_name: Option<String>,
    pub source_bundle_id: Option<String>,
    pub source_pid: Option<i64>,
    pub source_app_path: Option<String>,
    pub source_app_version: Option<String>,
    pub source_window_title: Option<String>,
    pub source_url: Option<String>,
    pub source_domain: Option<String>,
    pub source_tab_title: Option<String>,
    pub selection_rect: Option<SelectionRect>,
    pub capture_method: Option<String>,
    pub locale: Option<String>,
    pub input_language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteDeleteRequest {
    #[serde(rename = "favoriteId")]
    pub favorite_id: UUID,
}

#[tauri::command]
pub async fn favorite_remove_tag(
    request: FavoriteRemoveTagRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!(
        "[favorite_remove_tag] IPC command called for favorite_id: {}",
        request.favorite_id
    );

    match favorite_repo
        .remove_tag(&request.favorite_id, &request.tag_name)
        .await
    {
        Ok(()) => {
            tracing::info!("[favorite_remove_tag] Command completed successfully");
            Ok(())
        }
        Err(e) => {
            tracing::error!("[favorite_remove_tag] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_delete(
    request: FavoriteDeleteRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!(
        "[favorite_delete] IPC command called for favorite_id: {}",
        request.favorite_id
    );

    match favorite_repo.delete_by_id(&request.favorite_id).await {
        Ok(()) => {
            tracing::info!("[favorite_delete] Command completed successfully");
            Ok(())
        }
        Err(e) => {
            tracing::error!("[favorite_delete] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_create_external(
    request: FavoriteCreateExternalRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<UUID, AppError> {
    tracing::info!("[favorite_create_external] IPC command called");

    let FavoriteCreateExternalRequest {
        content,
        role,
        tags,
        note,
        context,
        selection_text_raw,
        source_app_name,
        source_bundle_id,
        source_pid,
        source_app_path,
        source_app_version,
        source_window_title,
        source_url,
        source_domain,
        source_tab_title,
        selection_rect,
        capture_method,
        locale,
        input_language,
    } = request;

    let content = content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::validation_error("选中的文本为空"));
    }

    let selection_rect = match selection_rect {
        Some(rect) => Some(serde_json::to_string(&rect).map_err(|e| {
            AppError::internal_error(&format!("Failed to serialize selection rect: {}", e))
        })?),
        None => None,
    };

    let source_domain = match (source_domain, source_url.as_deref()) {
        (Some(domain), _) => Some(domain),
        (None, Some(url)) => Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.domain().map(|d| d.to_string())),
        _ => None,
    };

    let create_request = CreateFavoriteRequest {
        message_id: uuid::Uuid::new_v4().to_string(),
        chat_id: EXTERNAL_CHAT_ID.to_string(),
        content,
        role,
        message_type: FavoriteMessageType::External,
        tags,
        note,
        context,
        selection_text_raw,
        source_app_name,
        source_bundle_id,
        source_pid,
        source_app_path,
        source_app_version,
        source_window_title,
        source_url,
        source_domain,
        source_tab_title,
        selection_rect,
        capture_method,
        locale,
        input_language,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match favorite_repo
        .create_external_favorite(&create_request)
        .await
    {
        Ok(favorite_id) => {
            tracing::info!(
                "[favorite_create_external] Command completed successfully, favorite_id: {}",
                favorite_id
            );
            Ok(favorite_id)
        }
        Err(e) => {
            tracing::error!("[favorite_create_external] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn favorite_save_text_ranges(
    request: FavoriteSaveTextRangesRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!(
        "[favorite_save_text_ranges] IPC command called for message_id: {}",
        request.message_id
    );

    if request.ranges.is_empty() {
        favorite_repo
            .delete_text_favorite(&request.message_id, &request.chat_id)
            .await?;
        return Ok(());
    }

    let content = serde_json::to_string(&request.ranges).map_err(|e| {
        AppError::internal_error(&format!("Failed to serialize text ranges: {}", e))
    })?;

    let create_request = CreateFavoriteRequest {
        message_id: request.message_id,
        chat_id: request.chat_id,
        content,
        role: request.role,
        message_type: FavoriteMessageType::Text,
        tags: request.tags,
        note: request.note,
        context: request.context,
        selection_text_raw: None,
        source_app_name: None,
        source_bundle_id: None,
        source_pid: None,
        source_app_path: None,
        source_app_version: None,
        source_window_title: None,
        source_url: None,
        source_domain: None,
        source_tab_title: None,
        selection_rect: None,
        capture_method: None,
        locale: None,
        input_language: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    favorite_repo.upsert_text_favorite(&create_request).await?;

    Ok(())
}
