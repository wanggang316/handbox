// HandBox Tauri 应用主入口

// 声明模块
pub mod commands;
pub mod config;
pub mod menu;
pub mod models;
pub mod services;
pub mod storage;
pub mod tray;
pub mod utils;

use crate::tray::setup_tray;

#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};

use crate::commands::*;
use crate::services::{
    selection::setup_selection, AgentProjectService, AgentRuntime, AgentService,
    AgentSessionService, ArtifactService, McpService, MessageService, ModelService,
    ProviderService, SearchService, SessionService, SettingsService, StorageService,
    UserSessionService, WordService,
};
use crate::storage::{ArtifactRepository, Database, FavoriteRepository, WordRepository};
use crate::utils::logger;
use std::sync::Arc;

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

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    #[cfg(target_os = "macos")]
    {
        // 初始化 NSPanel 插件
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            // 创建菜单
            let menu = crate::menu::create_menu(app.handle()).expect("Failed to create menu");
            app.set_menu(menu).expect("Failed to set menu");

            // Setup tray icon and menu
            if let Err(e) = setup_tray(app.handle()) {
                eprintln!("Failed to setup tray: {}", e);
            }

            // 创建选择面板 (NSPanel) - 必须在setup中同步创建
            #[cfg(target_os = "macos")]
            {
                if let Err(e) = setup_selection(&app.handle()) {
                    tracing::error!("Failed to setup selection panels: {e}");
                    eprintln!("Failed to setup selection panels: {e}");
                    // 不退出应用，因为选择面板是可选功能
                }
            }

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
        .on_menu_event(|app: &AppHandle, event| {
            crate::menu::handle_menu_event(app, event.id().as_ref());
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // 调试命令
            debug_check_file,
            // debug_show_selection_overlay,
            // 选择相关命令
            selection_hide_menu_panel,
            selection_show_content_panel,
            selection_hide_content_panel,
            selection_set_content_pinned,
            selection_get_content_pinned,
            selection_show_settings_panel,
            selection_hide_settings_panel,
            selection_disable_current_app_by_pid,
            selection_disable_current_app_by_bundle_id,
            selection_disable_global,
            selection_get_disabled_apps,
            selection_remove_disabled_app,
            // selection_hide_action_panel,
            // selection_show_action_panel,
            // // selection_overlay_hide,
            // selection_overlay_resize,
            // selection_overlay_lock,
            // selection_overlay_dismiss,
            // selection_overlay_set_interactive,
            // 认证相关命令
            auth_start_google_oauth,
            auth_google_login,
            auth_logout,
            auth_refresh_token,
            auth_get_user,
            auth_update_profile,
            auth_validate_token,
            // Session 相关命令 (原 chat 相关命令)
            session_create,
            session_list,
            session_get,
            session_update_field,
            session_update_model,
            session_clear_model_parameters,
            session_update_name,
            session_delete,
            session_generate_title,
            session_create_from_agent,
            // Agent 相关命令
            agent_create,
            agent_list,
            agent_get,
            agent_update_field,
            agent_update_name,
            agent_delete,
            // Agent Session（Agent 模式会话 CRUD）命令
            agent_session_create,
            agent_session_list,
            agent_session_get,
            agent_session_rename,
            agent_session_update_field,
            agent_session_delete,
            agent_session_messages,
            // Agent Project（按工作目录分组会话）命令
            agent_project_create,
            agent_project_list,
            agent_project_rename,
            agent_project_delete,
            // Agent 模式 run 命令
            agent_run_stream,
            agent_run_abort,
            agent_run_steer,
            // 消息相关命令
            message_user_send,
            message_user_send_stream,
            message_list,
            message_get,
            message_update,
            message_delete,
            message_assistant_regenerate_stream,
            message_user_resend_stream,
            message_stop_stream,
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
            model_add,
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
            // Skill 管理命令
            skill_list,
            // 设置相关命令
            settings_get,
            settings_update,
            settings_reset,
            settings_export,
            settings_import,
            settings_validate_mcp,
            settings_test_mcp_server,
            settings_system_info,
            // 单词相关命令
            word_create,
            word_list,
            word_get,
            word_update,
            word_delete,
            word_translation_history,
            // LLM 配置相关命令
            get_provider_configs,
            get_provider_config_by_type,
            hand_ai_list_providers,
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
            favorite_list_tags,
            favorite_save_text_ranges,
            favorite_add_tag,
            favorite_remove_tag,
            favorite_delete,
            favorite_create_external,
            // 辅助功能权限命令
            accessibility_check_permission,
            accessibility_request_permission,
            accessibility_open_settings,
            // 选择相关命令
            selection_show_content_panel,
            selection_hide_content_panel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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

    let llm_config_value = crate::config::llm_config::LlmConfig::load_from_app(app);
    crate::config::llm_config::install_global_llm_config(llm_config_value.clone());

    // 初始化各个服务
    let provider_service = ProviderService::new(database_service.clone());
    let provider_service_shared = Arc::new(provider_service.clone());

    let model_service = ModelService::new(database_service.clone());

    let mcp_service = McpService::new(database_service.clone());
    let mcp_service_shared = Arc::new(mcp_service.clone());

    let session_service =
        SessionService::new(database_service.clone(), provider_service_shared.clone());
    let session_service_shared = Arc::new(session_service.clone());

    let message_service = MessageService::new(
        database_service.clone(),
        provider_service_shared.clone(),
        session_service_shared,
        mcp_service_shared,
        storage_service.clone(),
    );

    let search_service = SearchService::new(database_service.clone(), storage_service.clone());

    let settings_service = SettingsService::new(storage_service.clone());

    let word_repo = Arc::new(WordRepository::new(database_service.clone()));
    let word_service = WordService::new(word_repo, settings_service.clone());

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

    // 初始化 Agent 服务
    let agent_service = AgentService::new(database_service.clone());

    // 初始化 Agent Session 服务（Agent 模式会话 CRUD）
    let agent_session_service = AgentSessionService::new(database_service.clone());

    // 初始化 Agent Project 服务（按工作目录分组 Agent 模式会话）
    let agent_project_service = AgentProjectService::new(database_service.clone());

    // 初始化 Skill 服务（解析三个 scope 根：app-data + user；project 按 run 解析）。
    // app-data: <app_data_dir>/skills；user: ~/.agents/skills（home_dir 解析失败时
    // 退回一个不存在的根，使 user scope 静默为空而非阻断启动）。
    let skill_appdata_root = data_dir.join("skills");
    let skill_user_root = app
        .path()
        .home_dir()
        .map(|home| home.join(".agents").join("skills"))
        .unwrap_or_else(|_| std::path::PathBuf::from("/nonexistent/handbox-skills/user"));
    let skill_service = Arc::new(crate::services::SkillService::new(
        skill_appdata_root,
        skill_user_root,
    ));

    // 初始化 Agent 运行时（Agent 模式 run 循环 + 事件发射 + 并发去重 + skill 注入）
    let agent_runtime =
        AgentRuntime::new_with_skills(database_service.clone(), skill_service.clone());

    // 将服务注册到应用状态
    app.manage(storage_service);
    app.manage(session_service);
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
    app.manage(agent_service);
    app.manage(agent_session_service);
    app.manage(agent_project_service);
    app.manage(agent_runtime);
    app.manage(skill_service);

    // Services are registered — the foreground can now read DB-cached data.
    // Catalog sync runs ENTIRELY in the background from here: prime the
    // in-memory catalog from the local cache, then refresh from hand-ai's
    // daily-published Release asset and every 24h. Kept off the startup
    // critical path so it never blocks the session / model list. Upstream
    // additions (e.g. OpenRouter's full tool-capable list incl. `~*-latest`
    // aliases) resolve at chat time once the refresh lands. No local synthesis.
    crate::services::catalog_sync::spawn();

    Ok(())
}
