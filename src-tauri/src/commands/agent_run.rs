// Agent 模式 run 相关 IPC 命令。
//
// `agent_run_stream` 启动一次 Agent 会话回合：装配一个把事件 `window.emit`
// 出去的 sink，委托给 `AgentRuntime::start_run`。run loop 在后台任务里驱动，
// 事件经 `agent_stream_event` 抵达前端，回合结束时经 `agent_stream_closed`
// 终结（每个 run 恰好一次）。
//
// 持久化 / abort / 错误分型 / tools 属于后续 feature，不在此命令层实现。

use std::sync::Arc;

use tauri::{Emitter, State, Window};

use crate::models::AppError;
use crate::services::{AgentRunRequest, AgentRuntime, RunSink};
use crate::storage::types::UUID;

/// 前端事件通道名：每条 AgentEvent 经此发出，payload 为 `{ sessionId, event }`。
const EVENT_NAME: &str = "agent_stream_event";
/// 前端事件通道名：回合终结信号，payload 为 `{ sessionId }`（每个 run 恰好一次）。
const CLOSED_NAME: &str = "agent_stream_closed";
/// 前端事件通道名：run-level 错误 envelope，payload 为
/// `{ sessionId, error: { code, message, hint } }`（在 closed **之前**发出）。
const ERROR_NAME: &str = "agent_stream_error";

/// 启动一次 Agent run（流式）。
///
/// 装配一个 Window-emitting 的 `RunSink` 后委托给 `AgentRuntime::start_run`。
/// `start_run` 在 spawn 后台 loop 后立即返回 —— 因此本命令也立即返回 `Ok(())`，
/// 真实输出经事件异步抵达。
///
/// 当该会话已有活跃 run 时（one-run-per-session），`start_run` 返回
/// `AGENT_RUN_ALREADY_ACTIVE` 错误，本命令将其透传给调用方。
#[tauri::command]
pub async fn agent_run_stream(
    request: AgentRunRequest,
    window: Window,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    let event_window = window.clone();
    let error_window = window.clone();
    let closed_window = window;

    let sink = RunSink::new(
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
    // `agent_stream_error` 发出（在 closed 之前）。未注入时它会退回 `on_event`，
    // 因此这一步是让 VAL-RUN-012 在运行时可观测的关键接线。
    .with_error(Arc::new(move |payload| {
        if let Err(e) = error_window.emit(ERROR_NAME, payload) {
            tracing::warn!("[agent_run_stream] failed to emit {}: {}", ERROR_NAME, e);
        }
    }));

    runtime
        .start_run(request.session_id, request.input, request.attachments, sink)
        .await
}

/// 中止某个 Agent 会话的活跃 run（流式回合）。
///
/// 从运行时注册表取出该会话的取消 token 并触发取消；hand-agent 的 loop 随之
/// 终止、合成一条 `stopReason=aborted` 的终结 assistant 消息（既有的 MessageEnd
/// 持久化路径会把它落库），并照常发出**恰好一次** `agent_stream_closed`。
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
/// 委托给 `AgentRuntime::steer`：从运行时注册表取出该会话进行中 run 的 steering
/// 队列并把这条 user 消息压入；hand-agent 的 loop 在下一个 turn 边界 drain 该队列
/// 并把消息注入 context（照常发出 `agent_stream_event` 并落库）。这保持
/// one-run-per-session —— steering 喂给既有 run，不启动并发 run。
///
/// 空 / 纯空白 `text` 是 no-op；该会话无活跃 run 时也是**干净的 no-op**
/// （返回 `Ok(())`，不报错）—— 前端可能在 run 刚自然结束时竞态地调用本命令。
#[tauri::command]
pub async fn agent_run_steer(
    session_id: UUID,
    text: String,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    runtime.steer(&session_id, text).await;
    Ok(())
}
