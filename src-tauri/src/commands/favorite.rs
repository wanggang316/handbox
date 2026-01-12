// 收藏相关 IPC 命令

use crate::models::AppError;
use crate::storage::types::{CreateFavoriteRequest, Favorite, FavoriteMessageType, UUID};
use crate::storage::FavoriteRepository;
use serde::{Deserialize, Serialize};
use tauri::State;

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
    pub tags: Vec<String>,
    pub note: Option<String>,
    #[serde(rename = "selectedText")]
    pub selected_text: Option<String>,
}

#[tauri::command]
pub async fn favorite_toggle(
    request: FavoriteToggleRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!("[favorite_toggle] IPC command called for message_id: {}", request.message_id);

    let message_type_enum = match request.message_type.as_str() {
        "text" => FavoriteMessageType::Text,
        "image" => FavoriteMessageType::Image,
        "chat" => FavoriteMessageType::Chat,
        "other" => FavoriteMessageType::Other,
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
        selected_text: request.selected_text,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match favorite_repo.toggle_favorite(&create_request).await {
        Ok(is_favorited) => {
            tracing::info!("[favorite_toggle] Command completed successfully, is_favorited: {}", is_favorited);
            Ok(is_favorited)
        }
        Err(e) => {
            tracing::error!("[favorite_toggle] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteIsFavoritedRequest {
    #[serde(rename = "messageId")]
    pub message_id: UUID,
}

#[tauri::command]
pub async fn favorite_is_favorited(
    request: FavoriteIsFavoritedRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!("[favorite_is_favorited] IPC command called for message_id: {}", request.message_id);

    match favorite_repo.is_favorited(&request.message_id).await {
        Ok(is_favorited) => {
            tracing::info!("[favorite_is_favorited] Command completed, result: {}", is_favorited);
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
            tracing::info!("[favorite_list] Command completed, returned {} favorites", favorites.len());
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
    tracing::info!("[favorite_list_by_chat] IPC command called for chat_id: {}", request.chat_id);

    match favorite_repo.get_favorites_by_chat(&request.chat_id).await {
        Ok(favorites) => {
            tracing::info!("[favorite_list_by_chat] Command completed, returned {} favorites", favorites.len());
            Ok(favorites)
        }
        Err(e) => {
            tracing::error!("[favorite_list_by_chat] Command failed: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteAddTagRequest {
    #[serde(rename = "favoriteId")]
    pub favorite_id: UUID,
    pub tag: String,
}

#[tauri::command]
pub async fn favorite_add_tag(
    request: FavoriteAddTagRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!("[favorite_add_tag] IPC command called for favorite_id: {}", request.favorite_id);

    match favorite_repo.add_tag(&request.favorite_id, &request.tag).await {
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

#[tauri::command]
pub async fn favorite_remove_tag(
    request: FavoriteAddTagRequest,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!("[favorite_remove_tag] IPC command called for favorite_id: {}", request.favorite_id);

    match favorite_repo.remove_tag(&request.favorite_id, &request.tag).await {
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
