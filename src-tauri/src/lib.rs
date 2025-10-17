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
    ChatService, McpService, MessageService, ModelService, ProviderService, StorageService,
    UserSessionService,
};
use crate::storage::Database;
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
        provider_service_shared,
        chat_service_shared,
        mcp_service_shared,
        llm_config_provider,
    );

    // 初始化用户会话服务
    let user_session_service = UserSessionService::new(database_service.clone());

    // 从数据库恢复上次的用户会话
    if let Err(e) = user_session_service.load_session_from_db().await {
        tracing::warn!("恢复用户会话失败: {:?}", e);
    }

    // 将服务注册到应用状态
    app.manage(storage_service);
    app.manage(chat_service);
    app.manage(message_service);
    app.manage(provider_service);
    app.manage(model_service);
    app.manage(mcp_service);
    app.manage(user_session_service);

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
            chat_update,
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
            provider_get_with_models,
            provider_create,
            provider_update,
            provider_delete,
            provider_toggle,
            // 模型相关命令
            model_list_by_provider,
            model_toggle,
            model_toggle_favorite,
            model_get_all_with_providers,
            model_get_favorites,
            model_get_available,
            // MCP 管理命令
            mcp_list_servers,
            mcp_create_server,
            mcp_update_server,
            mcp_delete_server,
            mcp_toggle_server,
            mcp_refresh_server,
            mcp_update_tool_enabled,
            // LLM 配置相关命令
            get_provider_configs,
            get_provider_config_by_type,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
