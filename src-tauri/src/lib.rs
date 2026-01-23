// HandBox Tauri 应用主入口

// 声明模块
pub mod commands;
pub mod config;
pub mod menu;
pub mod models;
pub mod services;
pub mod storage;
pub mod utils;

mod accessibility;

use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use core_graphics::event::{CGEventField, CGEventType, EventField};
use mouce::common::MouseEvent;
use mouce::{Mouse, MouseActions};
use swift_rs::swift;
#[cfg(target_os = "macos")]
use tauri::ActivationPolicy;
use tauri::{AppHandle, Emitter, Manager, Runtime, Wry};
use tauri_nspanel::{ManagerExt, panel}; // 导入 c_void

use crate::commands::*;
use crate::services::{
    selection_panel::setup_selection_panels, start_selection_observer, ArtifactService,
    ChatService, McpService, MessageService, ModelService, ProviderService, SearchService,
    SettingsService, StorageService, UserSessionService, WordService,
};
use crate::storage::{ArtifactRepository, Database, FavoriteRepository, WordRepository};
use crate::utils::logger;
use handbox_llm::config::LlmConfigProvider;
use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::sync::Arc;

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


fn show_floating_panel(handle: &tauri::AppHandle) {
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Ok(panel) = handle_clone.get_webview_panel("floating") {
            if !panel.is_visible() {
                // 使用 show_and_make_key 让面板成为 key window，这样才能接收鼠标事件
                panel.show();
            }
        }
    });
}

/// 隐藏浮动面板的辅助函数
fn hide_floating_panel(handle: &tauri::AppHandle) {
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Ok(panel) = handle_clone.get_webview_panel("floating") {
            if panel.is_visible() {
                let _ = panel.hide();
            }
        }
    });
}

pub fn setup_mouce_observer(app_handle: tauri::AppHandle) {
    let mut mouse = Mouse::new();
    let handle_clone = app_handle.clone();

    // 在独立线程中运行，因为 hook 是阻塞的
    std::thread::spawn(move || {
        // 使用 mouce 监听全局事件
        let _ = mouse.hook(Box::new(move |event| {
            match event {
                // 1. 滚动事件：直接触发隐藏
                mouce::common::MouseEvent::Scroll(_, _) => {
                    hide_floating_panel(&handle_clone);
                }
                // 2. 左键点击：如果是按下（Press），通常也需要隐藏
                mouce::common::MouseEvent::Press(mouce::common::MouseButton::Left) => {
                    hide_floating_panel(&handle_clone);
                }
                // 3. 左键松开：这是你划词逻辑的触发点
                mouce::common::MouseEvent::Release(mouce::common::MouseButton::Left) => {
                    trigger_selection_logic(&handle_clone);
                }
                mouce::common::MouseEvent::RelativeMove(x, y) => {
                    tracing::info!("======> x: {}, y: {}", x, y);
                }
                mouce::common::MouseEvent::AbsoluteMove(x, y) => {
                    tracing::info!("-----> x: {}, y: {}", x, y);
                }
                _ => {}
            }
        })).expect("无法启动 mouce hook");
    });
}

fn trigger_selection_logic(handle: &tauri::AppHandle) {
    // 检查功能是否启用
    if !is_selection_toolbar_enabled(handle) {
        return;
    }

    let mouse = Mouse::new();
    // 使用 mouce 获取当前位置，替代之前的 Swift 传参
    if let Ok((x, y)) = mouse.get_position() {
        let handle_clone = handle.clone();
        tauri::async_runtime::spawn(async move {

            if let Some(text) = accessibility::get_ax_selected_text() {
                tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);

                let _ = handle_clone.emit(
                    "global-selection",
                    serde_json::json!({
                        "text": text,
                        "x": x,
                        "y": y,
                        "app_info": { "name": "1", "bundle_id": "2", "pid": 123 }
                    }),
                );

                tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                show_floating_panel(&handle_clone);
            }
        });
    }
}

/// 检查选中文本工具栏功能是否启用
fn is_selection_toolbar_enabled(handle: &tauri::AppHandle) -> bool {
    let settings_service: tauri::State<'_, SettingsService> = handle.state();
    match settings_service.get_settings() {
        Ok(settings) => settings.quick_tools.show_toolbar_on_selection,
        Err(_) => false,
    }
}

pub fn setup_esc_monitor(handle: AppHandle<Wry>) {
    std::thread::spawn(move || {
        // 【关键修复 1】使用位移操作生成掩码
        // let mask = 1 << core_graphics::event::CGEventType::KeyDown as u64; 

        if let Ok(tap) = core_graphics::event::CGEventTap::new(
            core_graphics::event::CGEventTapLocation::HID,
            core_graphics::event::CGEventTapPlacement::HeadInsertEventTap,
            core_graphics::event::CGEventTapOptions::Default,
            vec![CGEventType::KeyDown],
            move |_, _, event| {
                // 【关键修复 2】使用正确的枚举字段名  aaaaa
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                tracing::info!("-----> key_code: {}", key_code);
                // ESCAPE 键码是 53，COMMAND 键码是 55
                // if key_code == 53 || key_code == 55 || key_code == 56 || key_code == 57 {
                    hide_floating_panel(&handle);
                // }
                None
            },
        ) {
            unsafe {
                let loop_source = tap.mach_port.create_runloop_source(0).expect("RunLoop Err");
                let current_loop = core_foundation::runloop::CFRunLoopGetCurrent();
                
                let source_ptr: *mut std::ffi::c_void = std::mem::transmute(loop_source);

                core_foundation::runloop::CFRunLoopAddSource(
                    current_loop, 
                    source_ptr as *mut _, 
                    core_foundation::runloop::kCFRunLoopCommonModes
                );
                
                tap.enable();
                core_foundation::runloop::CFRunLoopRun();
            }
        }
    });
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

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init());

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

            // 创建选择面板 (NSPanel) - 必须在setup中同步创建
            #[cfg(target_os = "macos")]
            {
                if let Err(e) = setup_selection_panels(&app.handle()) {
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


            setup_mouce_observer(app.handle().clone());
            setup_esc_monitor(app.handle().clone());

            // let floating = app.get_webview_window("floating").unwrap();
            // floating.show().unwrap();
            // floating.center().unwrap();

            Ok(())
        })
        .on_menu_event(|app: &AppHandle, event| {
            crate::menu::handle_menu_event(app, event.id().as_ref());
        })
        .invoke_handler(tauri::generate_handler![
            // 调试命令
            debug_check_file,
            // debug_show_selection_overlay,
            // // 选择相关命令
            // // selection_get_last_payload,
            // selection_hide_menu_panel,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// 旧的窗口配置函数已移除，现在使用 tauri-nspanel
