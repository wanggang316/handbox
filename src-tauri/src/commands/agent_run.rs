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
// `agent_run_abort` / `agent_run_steer` 现已切到 coding-agent 驱动路径：经
// `coding_agent_runtime` 的进程级运行句柄注册表（`drive_agent_run` 在驱动一轮时
// 注册、closed 时注销）翻转 cancel token / push steering 消息，对新驱动的 run 生效。
// 旧的 `AgentRuntime` 仍保留（chat/其它路径），将在后续 milestone（M4）退役。
//
// M3 起会话走 **JSONL 持久化**：经 `resume_session = <session_id>` 构造，
// transcript 落 `<app_data_dir>/sessions/<flattened-cwd>/<session_id>.jsonl`，
// 每轮 append 到同一文件（同一 HandBox 会话 → 同一 JSONL）。续聊上下文由该 JSONL
// 的 `build_context()` 还原；仅当 JSONL 尚无消息时回退 SQLite seed（覆盖 pre-M3
// 老会话首次续聊，老数据迁移是 m3-migration-core）。

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Emitter, Manager, State, Window};

use crate::models::AppError;
use crate::services::agent_permission::{
    respond_to_approval, ApprovalDecision, ApprovalEmitter, APPROVAL_REQUEST_EVENT,
};
use crate::services::coding_agent_session::{build_agent_session, config_from_rows};
use crate::services::{
    abort_run, drive_agent_run, images_from_attachments, steer_run, AgentRunRequest,
    AgentSessionService, CodingRunSink, ProviderService,
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
/// 前端事件通道名：会话生命周期信号（compaction / session-info），payload 为
/// `{ sessionId, kind, .. }`。与三条 run 通道并列、独立 —— 这些不是 run 事件，
/// 不进 `agent_stream_event` reducer，故不影响 closed-once 不变量。前端据 `kind`
/// 渲染「整理上下文中」指示并即时更新侧栏会话标题。
const LIFECYCLE_NAME: &str = "agent_session_lifecycle";

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

    // Approval emitter for the M2 PermissionExtension: a dangerous tool call
    // (write/edit/bash) pushes an `agent_approval_request`
    // `{ sessionId, callId, toolName, args, requestId }` to the frontend and
    // awaits the user's decision (answered via the `agent_approval_respond` IPC).
    // Wrap `window.emit` so the extension stays decoupled from Tauri.
    let approval_window = window.clone();
    let approval_emitter: ApprovalEmitter = Arc::new(move |payload| {
        if let Err(e) = approval_window.emit(APPROVAL_REQUEST_EVENT, payload) {
            tracing::warn!(
                "[agent_run_stream] failed to emit {}: {}",
                APPROVAL_REQUEST_EVENT,
                e
            );
        }
    });

    let mut session = build_agent_session(&config, Some(approval_emitter))?;

    // --- (3) 续聊上下文（M3: JSONL 为权威源）。
    //
    // `build_agent_session` 以 `resume_session = <session_id>` 构造会话，coding-agent
    // 在构造时已用该 JSONL 的 `build_context()` 还原历史进 in-memory context；本轮
    // `send_message` 产生的新消息会经 SessionManager APPEND 回同一 JSONL。因此对
    // **JSONL 已有历史**的会话，无需也不应再 seed（再 seed 会用 SQLite 覆盖掉 JSONL
    // 已还原的 context）。
    //
    // 仅当 JSONL **尚无消息**（message_count == 0）时，才回退到既有 SQLite transcript
    // seed 进 context —— 覆盖「老会话（pre-M3，只有 SQLite transcript、还没 JSONL）」
    // 首次在新引擎下续聊的场景，使其上下文不丢。这只填 in-memory context、不落 JSONL
    // （`set_messages` 不触碰 SessionManager 的文件），也不写 SQLite；老数据真正迁移
    // 到 JSONL 是后续 feature（m3-migration-core）。新会话（JSONL 有历史、SQLite 空）
    // 走 message_count > 0 分支，原样使用 JSONL 还原的 context。
    let jsonl_message_count = session.messages().len();
    if jsonl_message_count == 0 {
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
    }

    // --- (4) 装配一个 Window-emitting 的 sink 并驱动一轮 ---
    let event_window = window.clone();
    let error_window = window.clone();
    let closed_window = window.clone();
    let lifecycle_window = window.clone();

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
    }))
    // 会话生命周期信号（compaction / session-info）走独立通道
    // `agent_session_lifecycle`，与 run 三通道并列、不进 run reducer。
    .with_lifecycle(Arc::new(move |payload| {
        if let Err(e) = lifecycle_window.emit(LIFECYCLE_NAME, payload) {
            tracing::warn!(
                "[agent_run_stream] failed to emit {}: {}",
                LIFECYCLE_NAME,
                e
            );
        }
    }));

    // 在 IPC 边界校验图片附件（超大 / 超量 / 非图片静默丢弃），把存活图片
    // 转成 ImageContent 块；空集合走纯文本路径（本轮仍正常起）。
    let images = images_from_attachments(&request.attachments);

    Ok(drive_agent_run(
        session,
        session_id,
        request.input,
        images,
        sink,
    ))
}

/// 中止某个 Agent 会话的活跃 run（若有）。
///
/// 经 `coding_agent_runtime` 的进程级注册表取出该会话 run 的 cancel handle
/// （`RunDriveHandles.cancel`）并翻转 token —— 与传给 coding-agent `send_message`
/// 的是**同一个** token，故 agent loop 在下一个 await 边界解开、合成一条
/// `stopReason=aborted` 的终结回合，随后驱动任务在唯一的 closed emit site 发出
/// `agent_stream_closed`（closed-once 不变量在 abort 路径同样成立）。
///
/// 对未知 / 已结束的会话是**干净的 no-op**（返回 `Ok(())`，不报错）—— 前端可能
/// 在 run 刚自然结束时竞态地调用本命令。
#[tauri::command]
pub async fn agent_run_abort(session_id: UUID) -> Result<(), AppError> {
    abort_run(&session_id);
    Ok(())
}

/// 把一条 steering 消息并入某个 Agent 会话**正在进行**的 run。
///
/// 经 `coding_agent_runtime` 的进程级注册表取出该会话 run 的 steering handle
/// （`RunDriveHandles.steering`），把 `text` 作为一条 user `Message` push 进队列；
/// agent loop 在下一个 turn 边界经 `get_steering_messages` drain 它，使消息并入
/// **当前轮**（不另起并发 run，也不进 follow-up 队列在本轮后自动续跑）。
///
/// 空 / 纯空白 `text` 是 no-op；该会话无活跃 run 时也是**干净的 no-op**
/// （返回 `Ok(())`，不报错）。
#[tauri::command]
pub async fn agent_run_steer(session_id: UUID, text: String) -> Result<(), AppError> {
    steer_run(&session_id, text);
    Ok(())
}

/// 回灌一次工具审批决策（含作用域），唤醒正在 await 的 `PermissionExtension` 钩子。
///
/// 危险工具（write/edit/bash）调用时，`PermissionExtension` 发出
/// `agent_approval_request` 并 await 一个以 `request_id` 为键的 oneshot；前端
/// 弹窗（m2-approval-modal）回答后经本命令把决策回灌。`decision` 三态（作用域显式）：
///  - `deny` → 工具被 `Cancel`（模型收被拒结果）。
///  - `allow_once` → 本次允许（`Continue`），不记忆；同工具下次仍弹窗。
///  - `allow_always` → 本次允许且**本会话**记住该工具（按 session_id 键控的进程内
///    始终允许集），同会话同工具后续调用不再弹窗、直接 `Continue`。该集仅内存、不落
///    DB/文件 → 不跨会话、不跨重启。
///
/// 幂等：首个 response 生效；重复 / 未知 `request_id` 是**干净的 no-op**
/// （注册表里已无该条目，什么都不做、不报错）—— 前端可能因竞态重复回答，或回答
/// 一个已随 run 中止而消失的请求。
#[tauri::command]
pub async fn agent_approval_respond(
    request_id: String,
    decision: ApprovalDecision,
) -> Result<(), AppError> {
    respond_to_approval(&request_id, decision);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-run-per-session concurrency gate, exercised directly against the
    /// process-level `active_coding_runs` registry — the same check-and-insert
    /// `agent_run_stream` performs before assembling a session. Driving the gate
    /// through the registry (rather than a full Tauri command) keeps the test
    /// hermetic: no DB, no window, no network.
    ///
    /// Mirrors the command's claim/reject sequence:
    /// 1. the first run claims the session (placeholder inserted),
    /// 2. a SECOND start for the SAME session is rejected with
    ///    `AGENT_RUN_ALREADY_ACTIVE` — no second concurrent run is spawned
    ///    (VAL-CARUN-014),
    /// 3. after the run's closed-emit removes the entry, the session can be
    ///    claimed cleanly again (a subsequent turn is not wedged).
    ///
    /// Replicates the gate's check-then-insert against the shared registry so it
    /// stays faithful to `agent_run_stream` step (1) without invoking the async
    /// command (which needs Tauri `State`/`Window`). A fresh uuid keeps the test
    /// isolated from the process-global registry.
    fn try_claim(session_id: &UUID) -> Result<(), AppError> {
        let mut runs = active_coding_runs().lock().unwrap();
        if runs.contains(session_id) {
            return Err(AppError::with_hint(
                "AGENT_RUN_ALREADY_ACTIVE",
                &format!("a run is already active for session: {session_id}"),
                "请等待当前回合结束后再发送",
            ));
        }
        runs.insert(session_id.clone());
        Ok(())
    }

    fn release(session_id: &UUID) {
        active_coding_runs().lock().unwrap().remove(session_id);
    }

    #[test]
    fn second_concurrent_run_is_rejected_then_reclaimable_after_close() {
        let session_id = uuid::Uuid::new_v4().to_string();

        // (1) first run claims the session.
        try_claim(&session_id).expect("first run claims the session");

        // (2) a second start on the same session is rejected — no concurrent run.
        let err = try_claim(&session_id).expect_err("second concurrent run must be rejected");
        assert_eq!(err.code, "AGENT_RUN_ALREADY_ACTIVE");

        // (3) once the run's closed-emit releases the entry, the session is
        // claimable again — a later turn is not permanently wedged.
        release(&session_id);
        try_claim(&session_id).expect("session is reclaimable after the run closes");

        // Cleanup so the process-global registry is left empty for other tests.
        release(&session_id);
    }
}
