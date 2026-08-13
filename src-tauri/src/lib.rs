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
use tauri::AppHandle;
use tauri::Manager;

use crate::commands::*;
use crate::services::{
    selection::setup_selection, AgentProjectService, AgentService, AgentSessionService,
    GenUiService, HookRuleService, JobExecutor, JobScheduler, JobService, McpService, ModelService,
    ProviderService, SettingsService, StorageService, UserSessionService,
};
use crate::storage::Database;
use crate::utils::logger;
use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = dotenvy::dotenv() {
        // A missing .env file is not fatal; just log it.
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

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
        .plugin(tauri_plugin_process::init())
        // Desktop notifications: the job executor raises a macOS banner when a
        // job's continuous-failure count crosses the alert threshold. Registered
        // here so the executor's `AppHandle` can resolve the notification state.
        .plugin(tauri_plugin_notification::init())
        // Global shortcut: backs the Quick Action overlay's summon hotkey. The
        // plugin only stands up the manager here; the actual accelerator from
        // `quickAction.shortcut` is registered later in the async service-init
        // path (after SettingsService is available). No per-shortcut handler on
        // the builder — each shortcut carries its own handler via `on_shortcut`.
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Window state: the sizes in tauri.conf.json are first-launch defaults;
        // afterwards the main window reopens where the user left it. The panel
        // windows are denylisted because they are repositioned per invocation
        // (caret / cursor anchored) and must never be restored. VISIBLE is left
        // out of the flags so a restored state cannot fight the deliberate
        // hidden-until-first-paint startup, and FULLSCREEN because entering it
        // on a still-hidden window is unreliable on macOS.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .with_denylist(&[
                    "selection_menu",
                    "selection_content",
                    "selection_settings",
                    "quick_action",
                ])
                .build(),
        );

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder
        .setup(|app| {
            let menu = crate::menu::create_menu(app.handle()).expect("Failed to create menu");
            app.set_menu(menu).expect("Failed to set menu");

            if let Err(e) = setup_tray(app.handle()) {
                eprintln!("Failed to setup tray: {}", e);
            }

            // Selection panels (NSPanel) must be created synchronously in setup.
            #[cfg(target_os = "macos")]
            {
                if let Err(e) = setup_selection(app.handle()) {
                    tracing::error!("Failed to setup selection panels: {e}");
                    eprintln!("Failed to setup selection panels: {e}");
                    // Selection panels are optional; keep the app running.
                }

                // Quick Action overlay (NSPanel): created synchronously on the main
                // thread because to_panel relies on the quick_action window
                // pre-declared in tauri.conf.json.
                crate::services::selection::quick_action_panel::init_panel(app.handle());

                // Sidebar vibrancy: a behind-window NSVisualEffectView spanning the
                // whole (transparent) main window. The frontend keeps the content
                // card opaque, so only the sidebar column reads as translucent, and
                // it paints the sidebar opaque too when `general.sidebarVibrancy`
                // is off — the view is harmless behind opaque backgrounds, so it is
                // applied unconditionally here (settings are not yet loaded).
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(e) = window_vibrancy::apply_vibrancy(
                        &window,
                        window_vibrancy::NSVisualEffectMaterial::Sidebar,
                        None,
                        None,
                    ) {
                        tracing::warn!("Failed to apply sidebar vibrancy: {e}");
                    }

                }
            }

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = initialize_services(&app_handle).await {
                    eprintln!("Failed to initialize services: {e}");
                    std::process::exit(1);
                }
            });

            // The main window starts hidden (visible:false) and is shown by the
            // frontend after first paint to avoid a startup flash. Fallback: if the
            // frontend fails to boot, force-show after 4s so the app is never
            // windowless.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                    if let Some(w) = handle.get_webview_window("main") {
                        if !w.is_visible().unwrap_or(true) {
                            let _ = w.show();
                        }
                    }
                });
            }

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
            debug_check_file,
            // debug_show_selection_overlay,
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
            quick_action_show,
            quick_action_hide,
            quick_action_toggle,
            quick_action_register_shortcut,
            quick_action_unregister_shortcut,
            quick_action_continue_in_chat,
            // selection_hide_action_panel,
            // selection_show_action_panel,
            // // selection_overlay_hide,
            // selection_overlay_resize,
            // selection_overlay_lock,
            // selection_overlay_dismiss,
            // selection_overlay_set_interactive,
            auth_start_google_oauth,
            auth_google_login,
            auth_logout,
            auth_refresh_token,
            auth_get_user,
            auth_update_profile,
            auth_validate_token,
            agent_create,
            agent_list,
            agent_get,
            agent_update_field,
            agent_update_name,
            agent_delete,
            // GenUI: named JSON-Render UI spec CRUD
            genui_create,
            genui_list,
            genui_get,
            genui_update,
            genui_delete,
            agent_session_create,
            agent_session_create_from_definition,
            agent_session_reinstantiate_from_definition,
            agent_session_list,
            agent_session_get,
            agent_session_rename,
            agent_session_generate_title,
            agent_session_update_field,
            agent_session_set_pinned,
            agent_session_set_archived,
            agent_session_delete,
            agent_session_messages,
            // Agent projects: sessions grouped by working directory
            agent_project_create,
            agent_project_list,
            agent_project_rename,
            agent_project_delete,
            // "Open in ...": open a working directory in an external editor/terminal/Finder
            open_in_list_targets,
            open_in_open,
            agent_run_stream,
            agent_run_abort,
            agent_run_steer,
            agent_approval_respond,
            agent_question_respond,
            open_settings_window,
            provider_list,
            provider_get,
            provider_create,
            provider_update,
            provider_delete,
            provider_toggle,
            provider_list_with_models,
            model_list_by_provider,
            model_toggle,
            model_toggle_favorite,
            model_add,
            mcp_list_servers,
            mcp_create_server,
            mcp_update_server,
            mcp_delete_server,
            mcp_toggle_server,
            mcp_refresh_server,
            mcp_update_tool_enabled,
            hook_rule_list,
            hook_rule_create,
            hook_rule_update,
            hook_rule_delete,
            skill_list,
            skill_set_disabled,
            settings_get,
            settings_update,
            settings_reset,
            settings_export,
            settings_import,
            settings_validate_mcp,
            settings_test_mcp_server,
            settings_system_info,
            // LLM config (llm_config.json) lookups
            get_provider_configs,
            get_provider_config_by_type,
            hand_ai_list_providers,
            job_preview_schedule,
            job_create,
            job_list,
            job_get,
            job_update,
            job_delete,
            job_set_enabled,
            job_execution_list,
            job_run_now,
            clipboard_copy_image,
            image_proxy,
            accessibility_check_permission,
            accessibility_request_permission,
            accessibility_open_settings,
            selection_show_content_panel,
            selection_hide_content_panel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn initialize_services(
    app: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    let storage_service = Arc::new(StorageService::new(data_dir.clone())?);

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

    let db_path = storage_service.get_database_path();
    let database_service = Arc::new(
        Database::new(&db_path)
            .await
            .map_err(|e| format!("Failed to initialize database: {e}"))?,
    );

    let llm_config_value = crate::config::llm_config::LlmConfig::load_from_app(app);
    crate::config::llm_config::install_global_llm_config(llm_config_value.clone());

    let provider_service = ProviderService::new(database_service.clone());
    let provider_service_shared = Arc::new(provider_service.clone());

    let model_service = ModelService::new(database_service.clone());

    let mcp_service = McpService::new(database_service.clone());

    let hook_rule_service = HookRuleService::new(database_service.clone());

    let settings_service = SettingsService::new(storage_service.clone());

    // Register the Quick Action global hotkey from the persisted
    // `quickAction.shortcut`. This runs in the async service-init path (after
    // SettingsService exists) — an early-launch press before this completes is
    // an accepted no-op. A failed registration is logged and swallowed so the
    // app still launches; the structured AppError surfaces via the re-register
    // command instead.
    #[cfg(target_os = "macos")]
    {
        match settings_service.get_settings() {
            Ok(settings) => {
                if !settings.quick_action.enabled {
                    tracing::info!(
                        "[QuickActionShortcut::register] quick action disabled, skipping hotkey registration"
                    );
                } else if let Err(e) = crate::services::quick_action::register_shortcut(
                    app,
                    &settings.quick_action.shortcut,
                ) {
                    tracing::error!(
                        "[QuickActionShortcut::register] startup registration failed (continuing): {e}"
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    "[QuickActionShortcut::register] could not read settings for hotkey (continuing): {e}"
                );
            }
        }
    }

    let user_session_service = UserSessionService::new(database_service.clone());

    if let Err(e) = user_session_service.load_session_from_db().await {
        tracing::warn!("恢复用户会话失败: {:?}", e);
    }

    let agent_service = AgentService::new(database_service.clone());

    // GenUI: CRUD for named JSON-Render UI specs.
    let genui_service = GenUiService::new(database_service.clone());

    let agent_session_service = AgentSessionService::new(database_service.clone());

    // Agent projects group agent-mode sessions by working directory.
    let agent_project_service = AgentProjectService::new(database_service.clone());

    // Skill scopes: app-data (<app_data_dir>/skills) + user (~/.agents/skills);
    // the project scope resolves per run. On home_dir failure, fall back to a
    // nonexistent root so the user scope is silently empty instead of blocking
    // startup. Built before the job executor, whose AgentRuntime needs it.
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

    // One-time materialization of legacy SQLite agent transcripts into JSONL;
    // on success the redundant `agent_session_messages` table is dropped. Gated
    // on that table's existence: present => migrate + drop; absent => done,
    // skip entirely (no per-startup rescan). Only the transcript table is
    // dropped — `agent_sessions` / `agent_projects` remain the live config +
    // grouping source, never drop them.
    //
    // Must finish before any run command executes, or an old session's first
    // run would persist only the new turn and lose its history. data_dir
    // doubles as the JSONL base and the cwd fallback for sessions without a
    // working_dir (matching config_from_rows / session_cwd on the write side).
    // A failed migration is logged without blocking startup and does NOT drop
    // the table, so transcripts are preserved.
    match crate::services::migrate_and_drop_legacy_if_present(database_service.clone(), &data_dir)
        .await
    {
        Ok(report) => {
            if let Some(migration) = report.migration {
                tracing::info!(
                    migrated = migration.migrated_sessions,
                    messages = migration.messages_migrated,
                    rewritten = migration.rewritten_sessions,
                    skipped_existing = migration.skipped_existing,
                    skipped_empty = migration.skipped_empty,
                    skipped_undeserializable = migration.skipped_undeserializable,
                    dropped_legacy_table = report.dropped,
                    "migrated legacy SQLite agent transcripts to JSONL and dropped the legacy table"
                );
            }
        }
        Err(e) => {
            tracing::error!("SQLite→JSONL agent transcript migration failed: {:?}", e);
        }
    }

    let job_service = JobService::from_db(database_service.clone());

    // Runs one job and persists the result; the AppHandle lets it emit
    // `job_executed` on start/finish for live frontend refresh. Prompt target:
    // headless fresh chat, non-streaming send, provider pre-validated. Agent
    // target: headless fresh agent session driven one coding-agent round,
    // classified from its JSONL transcript; it reuses the foreground services,
    // with data_dir as the coding-agent base_dir / cwd fallback (the background
    // executor has no Window to resolve app_data_dir through).
    let job_executor = JobExecutor::from_db(database_service.clone())
        .with_app_handle(app.clone())
        .with_agent_services(
            Arc::new(agent_service.clone()),
            Arc::new(agent_session_service.clone()),
            provider_service_shared.clone(),
            Arc::new(hook_rule_service.clone()),
            data_dir.clone(),
        );

    // Background tick loop that fires due jobs. Cloning the executor clones its
    // Arc fields, not the services themselves.
    let job_scheduler = JobScheduler::from_db(database_service.clone(), job_executor.clone());

    app.manage(storage_service);
    app.manage(provider_service);
    app.manage(model_service);
    app.manage(mcp_service);
    app.manage(hook_rule_service);
    app.manage(settings_service);
    app.manage(user_session_service);
    app.manage(agent_service);
    app.manage(genui_service);
    app.manage(agent_session_service);
    app.manage(agent_project_service);
    app.manage(skill_service);
    app.manage(job_service);
    app.manage(job_executor);
    app.manage(job_scheduler.clone());

    // Services are registered — the foreground can now read DB-cached data.
    // Catalog sync runs ENTIRELY in the background from here: prime the
    // in-memory catalog from the local cache, then refresh from hand-ai's
    // daily-published Release asset and every 24h. Kept off the startup
    // critical path so it never blocks the session / model list. Upstream
    // additions (e.g. OpenRouter's full tool-capable list incl. `~*-latest`
    // aliases) resolve at chat time once the refresh lands. No local synthesis.
    crate::services::catalog_sync::spawn();

    // Start job scheduling. Jobs run only while the app runs: missed triggers
    // are never caught up; next_run_at is recomputed from "now" on restart.
    // Order: (1) reconcile `running` execution rows left by a previous process
    // (no live process — mark failed); (2) recompute next_run_at for every
    // enabled job, taking only the next cron occurrence > now (overdue jobs do
    // not fire at startup); (3) start the fixed 30s tick loop (DB is the single
    // source of truth, re-read every tick). Reconcile/recompute failures warn
    // but never block startup or the tick loop.
    {
        let scheduler = job_scheduler;
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.reconcile_stale_running().await {
                tracing::warn!("[JobScheduler] startup reconcile failed (continuing): {e:?}");
            }
            if let Err(e) = scheduler.recompute_all_enabled().await {
                tracing::warn!("[JobScheduler] startup recompute failed (continuing): {e:?}");
            }
            scheduler.spawn_tick_loop();
        });
    }

    Ok(())
}
