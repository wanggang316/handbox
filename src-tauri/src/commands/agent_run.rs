// Agent 模式 run 相关 IPC 命令。
//
// `agent_run_stream` 启动一次 Agent 会话回合，现在经 **coding-agent
// `AgentSession`** 驱动（重平台化的核心证明路径）：从 session_id 装配一个
// coding-agent session、订阅其事件、在后台任务里 `send_message` 跑一轮，并把
// `AgentSessionEvent` 映射到现有三个 Tauri 通道 —— `agent_stream_event`
// （`{ sessionId, event }`）、`agent_stream_closed`（`{ sessionId }`，每个 run
// 恰好一次）、`agent_stream_error`（`{ sessionId, error }`，在 closed 之前）。
// 前端契约（事件名 / payload 形状 / closed-once / 错误分型）保持不变。
//
// 旧的 `AgentRuntime` 驱动路径仍保留（`agent_run_abort` / `agent_run_steer`
// 暂仍委托它），将在后续 milestone 退役。steer / abort 接到 coding-agent
// session 的完整实现是下一个 feature（m1-steer-abort）：本命令在驱动一轮时已
// 拿到 `RunDriveHandles`（cancel / steering handle），为那个 feature 预留了
// 接线点。
//
// 本 feature 走**纯内存**会话（`no_session = true`）：既有 HandBox transcript
// 被 seed 进 context 以保证续聊上下文正确，但本轮新消息的 HandBox DB 持久化是
// M3，不在此实现。

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Emitter, Manager, State, Window};

use crate::models::AppError;
use crate::services::coding_agent_session::{build_agent_session, config_from_rows};
use crate::services::{
    drive_agent_run, AgentRunRequest, AgentRuntime, AgentSessionService, CodingRunSink,
    ProviderService,
};
use crate::storage::types::UUID;
use hand_ai_model::Message;

/// 前端事件通道名：每条 AgentEvent 经此发出，payload 为 `{ sessionId, event }`。
const EVENT_NAME: &str = "agent_stream_event";
/// 前端事件通道名：回合终结信号，payload 为 `{ sessionId }`（每个 run 恰好一次）。
const CLOSED_NAME: &str = "agent_stream_closed";
/// 前端事件通道名：run-level 错误 envelope，payload 为
/// `{ sessionId, error: { code, message, hint } }`（在 closed **之前**发出）。
const ERROR_NAME: &str = "agent_stream_error";

/// 进程级 one-run-per-session 注册表（coding-agent 驱动路径）。
///
/// coding-agent 的 `AgentSession` 在 `send_message` 期间独占 `&mut self`，被后台
/// 任务拥有，无处挂载实例级注册表（命令层每次调用拿到的是新的 `State` 引用）。
/// 用一个进程级 `HashSet<session_id>` 做并发去重：同一会话已有活跃 coding-agent
/// run 时，第二个 `agent_run_stream` 以 `AGENT_RUN_ALREADY_ACTIVE` 拒绝，不启动
/// 并发 run。条目在 run 终结（closed）时由发起任务移除 —— 与 closed 同一时点。
fn active_coding_runs() -> &'static Mutex<HashSet<UUID>> {
    static RUNS: OnceLock<Mutex<HashSet<UUID>>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 启动一次 Agent run（流式）—— 经 coding-agent `AgentSession` 驱动。
///
/// 步骤：
/// 1. one-run-per-session：进程级注册表已含该会话时以 `AGENT_RUN_ALREADY_ACTIVE`
///    拒绝；否则占位插入。
/// 2. 加载 session 行 + provider 行，装配 `HandBoxAgentSessionConfig`，构造一个
///    coding-agent `AgentSession`（纯内存）。
/// 3. seed 既有 HandBox transcript 进 session 的 context（续聊上下文）。
/// 4. 装配一个把事件 `window.emit` 出去的 `CodingRunSink`，委托给 `drive_agent_run`
///    —— 它 spawn 后台任务驱动一轮、把事件映射到三通道、保证 closed 恰好一次。
/// 5. spawn 一个看护任务：在驱动任务结束后把会话从注册表移除（与 closed 同步）。
///
/// `drive_agent_run` 是非阻塞的 —— spawn 后台任务后即返回，因此本命令也立即返回
/// `Ok(())`，真实输出经事件异步抵达。
///
/// 装配阶段（步骤 2/3）的任何错误都会先把注册表占位移除再向上抛出，避免会话被
/// 永久“卡住”。
#[tauri::command]
pub async fn agent_run_stream(
    request: AgentRunRequest,
    window: Window,
    sessions: State<'_, AgentSessionService>,
    providers: State<'_, ProviderService>,
) -> Result<(), AppError> {
    let session_id = request.session_id.clone();

    // --- (1) one-run-per-session：检查 + 占位插入 ---
    {
        let mut runs = active_coding_runs().lock().unwrap();
        if runs.contains(&session_id) {
            return Err(AppError::with_hint(
                "AGENT_RUN_ALREADY_ACTIVE",
                &format!("a run is already active for session: {}", session_id),
                "请等待当前回合结束后再发送",
            ));
        }
        runs.insert(session_id.clone());
    }

    // 从此处起，任何提前返回都必须先把占位移除。
    match assemble_and_drive(request, &window, &sessions, &providers).await {
        Ok(handles) => {
            // 看护任务：驱动任务结束（即 closed 已发出）后把会话从注册表移除。
            // 与 closed 同步 —— 移除发生在终结信号之后，使下一轮可以发起。
            let cleanup_session = session_id;
            tokio::spawn(async move {
                let _ = handles.task.await;
                active_coding_runs()
                    .lock()
                    .unwrap()
                    .remove(&cleanup_session);
            });
            Ok(())
        }
        Err(e) => {
            active_coding_runs().lock().unwrap().remove(&session_id);
            Err(e)
        }
    }
}

/// 装配 coding-agent session 并驱动一轮（不含注册表占位/清理 —— 由调用方管理）。
///
/// 拆出独立函数让 `agent_run_stream` 的占位清理（失败回滚）保持简单：装配阶段
/// 失败时调用方统一移除占位。返回 `RunDriveHandles`（含驱动任务 + 为下个 feature
/// 预留的 cancel / steering handle）。
async fn assemble_and_drive(
    request: AgentRunRequest,
    window: &Window,
    sessions: &AgentSessionService,
    providers: &ProviderService,
) -> Result<crate::services::RunDriveHandles, AppError> {
    let session_id = request.session_id.clone();

    // --- (2) 加载 session 行 + provider 行 ---
    let session_row = sessions.get_session(session_id.clone()).await?;
    let provider_id = session_row
        .provider_id
        .clone()
        .ok_or_else(|| AppError::validation_error("agent session has no provider_id selected"))?;
    let provider = providers.get_provider(&provider_id).await?;

    // app_data_dir 作为 session 的 base_dir（sandbox 持久化根）与 working_dir
    // 缺省时的 cwd 后备。经 Tauri PathResolver 解析。
    let app_data_dir =
        window.app_handle().path().app_data_dir().map_err(|e| {
            AppError::internal_error(&format!("failed to resolve app data dir: {e}"))
        })?;

    let config = config_from_rows(&session_row, &provider, app_data_dir)?;
    let mut session = build_agent_session(&config)?;

    // --- (3) seed 既有 HandBox transcript 进 context（续聊上下文）。
    // 每条 payload 反序列化为 hand-agent `Message`；与旧路径的 seeding 一致。
    // 本 feature 纯内存：seeding 只填 in-memory context，不落 HandBox DB（M3）。
    let history = sessions.list_messages(session_id.clone()).await?;
    if !history.is_empty() {
        let mut seeded: Vec<Message> = Vec::with_capacity(history.len());
        for row in history {
            let msg: Message = serde_json::from_value(row.payload).map_err(|e| {
                AppError::internal_error(&format!(
                    "agent transcript payload (seq {}) is not a valid hand-agent Message: {}",
                    row.seq, e
                ))
            })?;
            seeded.push(msg);
        }
        session.set_messages(seeded);
    }

    // --- (4) 装配一个 Window-emitting 的 sink 并驱动一轮 ---
    let event_window = window.clone();
    let error_window = window.clone();
    let closed_window = window.clone();

    let sink = CodingRunSink::new(
        Arc::new(move |payload| {
            if let Err(e) = event_window.emit(EVENT_NAME, payload) {
                tracing::warn!("[agent_run_stream] failed to emit {}: {}", EVENT_NAME, e);
            }
        }),
        Arc::new(move |payload| {
            if let Err(e) = closed_window.emit(CLOSED_NAME, payload) {
                tracing::warn!("[agent_run_stream] failed to emit {}: {}", CLOSED_NAME, e);
            }
        }),
    )
    // run-level `Err` envelope 走专用通道，作为一个 DISTINCT 的窗口事件
    // `agent_stream_error` 发出（在 closed 之前）。
    .with_error(Arc::new(move |payload| {
        if let Err(e) = error_window.emit(ERROR_NAME, payload) {
            tracing::warn!("[agent_run_stream] failed to emit {}: {}", ERROR_NAME, e);
        }
    }));

    Ok(drive_agent_run(session, session_id, request.input, sink))
}

/// 中止某个 Agent 会话的活跃 run（若有）。
///
/// 目前仍委托旧的 `AgentRuntime`。把 abort 接到 coding-agent session 的
/// cancel handle（`RunDriveHandles.cancel`）是下一个 feature（m1-steer-abort）。
///
/// 对未知 / 已结束的会话是**干净的 no-op**（返回 `Ok(())`，不报错）—— 前端可能
/// 在 run 刚自然结束时竞态地调用本命令。
#[tauri::command]
pub async fn agent_run_abort(
    session_id: UUID,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    runtime.abort(&session_id).await;
    Ok(())
}

/// 把一条 steering 消息并入某个 Agent 会话**正在进行**的 run。
///
/// 目前仍委托旧的 `AgentRuntime`。把 steer 接到 coding-agent session 的
/// steering handle（`RunDriveHandles.steering`）是下一个 feature（m1-steer-abort）。
///
/// 空 / 纯空白 `text` 是 no-op；该会话无活跃 run 时也是**干净的 no-op**
/// （返回 `Ok(())`，不报错）。
#[tauri::command]
pub async fn agent_run_steer(
    session_id: UUID,
    text: String,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    runtime.steer(&session_id, text).await;
    Ok(())
}
