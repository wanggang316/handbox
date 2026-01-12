// 收藏相关 IPC 命令

use crate::models::AppError;
use crate::storage::types::{CreateFavoriteRequest, Favorite, FavoriteMessageType, UUID};
use crate::storage::FavoriteRepository;
use tauri::State;

#[tauri::command]
pub async fn favorite_toggle(
    message_id: UUID,
    chat_id: UUID,
    content: String,
    role: String,
    message_type: String,
    tags: Vec<String>,
    note: Option<String>,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!("[favorite_toggle] IPC command called for message_id: {}", message_id);

    let message_type_enum = match message_type.as_str() {
        "text" => FavoriteMessageType::Text,
        "image" => FavoriteMessageType::Image,
        "chat" => FavoriteMessageType::Chat,
        "other" => FavoriteMessageType::Other,
        _ => FavoriteMessageType::Message,
    };

    let request = CreateFavoriteRequest {
        message_id,
        chat_id,
        content,
        role,
        message_type: message_type_enum,
        tags,
        note,
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    match favorite_repo.toggle_favorite(&request).await {
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

#[tauri::command]
pub async fn favorite_is_favorited(
    message_id: UUID,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<bool, AppError> {
    tracing::info!("[favorite_is_favorited] IPC command called for message_id: {}", message_id);

    match favorite_repo.is_favorited(&message_id).await {
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

#[tauri::command]
pub async fn favorite_list_by_chat(
    chat_id: UUID,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<Vec<Favorite>, AppError> {
    tracing::info!("[favorite_list_by_chat] IPC command called for chat_id: {}", chat_id);

    match favorite_repo.get_favorites_by_chat(&chat_id).await {
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

#[tauri::command]
pub async fn favorite_add_tag(
    favorite_id: UUID,
    tag: String,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!("[favorite_add_tag] IPC command called for favorite_id: {}", favorite_id);

    match favorite_repo.add_tag(&favorite_id, &tag).await {
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
    favorite_id: UUID,
    tag: String,
    favorite_repo: State<'_, FavoriteRepository>,
) -> Result<(), AppError> {
    tracing::info!("[favorite_remove_tag] IPC command called for favorite_id: {}", favorite_id);

    match favorite_repo.remove_tag(&favorite_id, &tag).await {
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
