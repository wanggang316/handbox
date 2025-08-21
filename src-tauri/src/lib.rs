// HandBox Tauri 应用主入口

// 声明模块
pub mod commands;
pub mod config;
pub mod menu;
pub mod models;
pub mod services;
pub mod utils;

use crate::commands::*;
use crate::config::AppConfig;
use crate::services::{
    ArtifactService, ChatService, ProviderService, SearchService, SettingsService, StorageService,
};
use std::sync::Arc;
use tauri::Manager;

/// 应用状态管理
pub struct AppState {
    pub config: AppConfig,
}

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

    // 初始化数据库
    storage_service
        .init_database()
        .await
        .map_err(|e| format!("Failed to initialize database: {e}"))?;

    // 初始化各个服务
    let chat_service = ChatService::new(storage_service.clone());
    let provider_service = ProviderService::new(storage_service.clone());
    let artifact_service = ArtifactService::new(storage_service.clone());
    let settings_service = SettingsService::new(storage_service.clone());
    let search_service = SearchService::new(storage_service.clone());

    // 将服务注册到应用状态
    app.manage(storage_service);
    app.manage(chat_service);
    app.manage(provider_service);
    app.manage(artifact_service);
    app.manage(settings_service);
    app.manage(search_service);

    // 加载应用配置
    let config_path = data_dir.join("app_config.json");
    let config = AppConfig::load_from_file(&config_path)?;
    app.manage(AppState { config });

    Ok(())
}

// 保留原始的 greet 命令用于测试
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            // 聊天相关命令
            chat_send,
            chat_create_session,
            chat_list_sessions,
            chat_get_session,
            chat_update_session,
            chat_delete_session,
            chat_get_messages,
            chat_update_message,
            chat_delete_message,
            chat_regenerate_message,
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
            provider_probe,
            provider_list_models,
            provider_toggle,
            provider_toggle_model,
            provider_get_available_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
