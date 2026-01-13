// HandBox Tauri 应用主入口

// 声明模块
pub mod commands;
pub mod config;
pub mod menu;
pub mod models;
pub mod services;
pub mod storage;
pub mod utils;

use crate::commands::*;
use crate::services::{
    ArtifactService, ChatService, McpService, MessageService, ModelService, ProviderService,
    SearchService, SettingsService, StorageService, UserSessionService, WordService,
};
use crate::storage::{ArtifactRepository, Database, FavoriteRepository, WordRepository};
use crate::utils::logger;
use handbox_llm::config::LlmConfigProvider;
use std::sync::Arc;
use tauri::Manager;

/// 初始化服务
async fn initialize_services(
    app: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 获取应用数据目录
    let data_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    // 初始化存储服务
    let storage_service = Arc::new(StorageService::new(data_dir.clone())?);

    // 允许前端通过 asset protocol 访问生成的媒体目录
    let media_root = data_dir.join("generated_media");
    std::fs::create_dir_all(&media_root)
        .map_err(|e| format!("Failed to create generated media directory: {e}"))?;
    app.asset_protocol_scope()
        .allow_directory(&media_root, true)
        .map_err(|e| format!("Failed to allow asset protocol for generated media: {e}"))?;

    let attachments_root = data_dir.join("message_attachments");
    std::fs::create_dir_all(&attachments_root)
        .map_err(|e| format!("Failed to create attachment directory: {e}"))?;
    app.asset_protocol_scope()
        .allow_directory(&attachments_root, true)
        .map_err(|e| format!("Failed to allow asset protocol for attachments: {e}"))?;

    // 初始化数据库服务
    let db_path = storage_service.get_database_path();
    let database_service = Arc::new(
        Database::new(&db_path)
            .await
            .map_err(|e| format!("Failed to initialize database: {e}"))?,
    );

    let llm_config = Arc::new(crate::config::llm_config::LlmConfig::load());
    let llm_config_provider: Arc<dyn LlmConfigProvider> = llm_config.clone();

    // 初始化各个服务
    let provider_service =
        ProviderService::new(database_service.clone(), llm_config_provider.clone());
    let provider_service_shared = Arc::new(provider_service.clone());

    let model_service = ModelService::new(database_service.clone(), llm_config_provider.clone());

    let mcp_service = McpService::new(database_service.clone());
    let mcp_service_shared = Arc::new(mcp_service.clone());

    let chat_service = ChatService::new(
        database_service.clone(),
        provider_service_shared.clone(),
        llm_config_provider.clone(),
    );
    let chat_service_shared = Arc::new(chat_service.clone());

    let message_service = MessageService::new(
        database_service.clone(),
        provider_service_shared.clone(),
        chat_service_shared,
        mcp_service_shared,
        storage_service.clone(),
        llm_config_provider.clone(),
    );

    let search_service = SearchService::new(database_service.clone(), storage_service.clone());

    let settings_service = SettingsService::new(storage_service.clone());

    let word_repo = Arc::new(WordRepository::new(database_service.clone()));
    let word_service = WordService::new(
        word_repo,
        provider_service_shared.clone(),
        settings_service.clone(),
        llm_config_provider.clone(),
    );

    // 初始化用户会话服务
    let user_session_service = UserSessionService::new(database_service.clone());

    // 从数据库恢复上次的用户会话
    if let Err(e) = user_session_service.load_session_from_db().await {
        tracing::warn!("恢复用户会话失败: {:?}", e);
    }

    // 初始化 Artifact 服务
    let artifact_repo = Arc::new(ArtifactRepository::new(database_service.clone()));
    let artifact_service = ArtifactService::new(artifact_repo, app.clone());

    // 初始化 Favorite 服务
    let favorite_repo = FavoriteRepository::new(database_service.clone());

    // 将服务注册到应用状态
    app.manage(storage_service);
    app.manage(chat_service);
    app.manage(message_service);
    app.manage(provider_service);
    app.manage(model_service);
    app.manage(mcp_service);
    app.manage(search_service);
    app.manage(settings_service);
    app.manage(word_service);
    app.manage(user_session_service);
    app.manage(artifact_service);
    app.manage(favorite_repo);

    Ok(())
}

// 保留原始的 greet 命令用于测试
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 加载环境变量
    if let Err(e) = dotenvy::dotenv() {
        // .env 文件不存在不是致命错误，只记录日志
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    // 初始化日志系统
    if let Err(e) = logger::init_logger() {
        eprintln!("Failed to initialize logger: {}", e);
    } else {
        tracing::info!("Logger initialized successfully");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 创建菜单
            let menu = crate::menu::create_menu(app.handle()).expect("Failed to create menu");
            app.set_menu(menu).expect("Failed to set menu");

            // 异步初始化服务
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = initialize_services(&app_handle).await {
                    eprintln!("Failed to initialize services: {e}");
                    std::process::exit(1);
                }
            });

            Ok(())
        })
        .on_menu_event(|app, event| {
            crate::menu::handle_menu_event(app, event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![
            // 测试命令
            greet,
            // 调试命令
            debug_check_file,
            // 认证相关命令
            auth_start_google_oauth,
            auth_google_login,
            auth_logout,
            auth_refresh_token,
            auth_get_user,
            auth_update_profile,
            auth_validate_token,
            // 聊天相关命令
            chat_create,
            chat_list,
            chat_get,
            chat_update_field,
            chat_update_model,
            chat_clear_model_parameters,
            chat_update_name,
            chat_delete,
            chat_generate_title,
            // 消息相关命令
            message_user_send,
            message_user_send_stream,
            message_list,
            message_get,
            message_update,
            message_delete,
            message_assistant_regenerate_stream,
            message_user_resend_stream,
            // message_execute_mcp_call, // Temporarily removed
            message_execute_tool_calls,
            message_execute_tool_calls_stream,
            // 窗口管理命令
            open_settings_window,
            close_settings_window,
            toggle_settings_window,
            // 供应商相关命令
            provider_list,
            provider_get,
            provider_create,
            provider_update,
            provider_delete,
            provider_toggle,
            provider_count_chats,
            provider_list_with_models,
            // 模型相关命令
            model_list_by_provider,
            model_toggle,
            model_toggle_favorite,
            model_count_chats,
            // MCP 管理命令
            mcp_list_servers,
            mcp_create_server,
            mcp_update_server,
            mcp_delete_server,
            mcp_toggle_server,
            mcp_refresh_server,
            mcp_update_tool_enabled,
            mcp_count_chats_using_server,
            mcp_remove_server_from_chats,
            // 设置相关命令
            settings_get,
            settings_update,
            settings_reset,
            settings_export,
            settings_import,
            settings_validate_mcp,
            settings_test_mcp_server,
            settings_system_info,
            settings_check_updates,
            // 单词相关命令
            word_create,
            word_list,
            word_get,
            word_update,
            word_delete,
            word_review,
            word_translate,
            word_lookup_record,
            word_lookup_history,
            word_lookup_delete,
            // LLM 配置相关命令
            get_provider_configs,
            get_provider_config_by_type,
            // 搜索相关命令
            search_query,
            search_history,
            search_add_history,
            search_clear_history,
            search_suggestions,
            // Artifact 相关命令
            artifact_create,
            artifact_update,
            artifact_get,
            artifact_list,
            artifact_delete,
            artifact_install,
            artifact_execute,
            artifact_init_builtin,
            // 剪贴板相关命令
            clipboard_copy_image,
            // 图片相关命令
            image_proxy,
            // 收藏相关命令
            favorite_toggle,
            favorite_is_favorited,
            favorite_list,
            favorite_list_by_chat,
            favorite_save_text_ranges,
            favorite_add_tag,
            favorite_remove_tag,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
