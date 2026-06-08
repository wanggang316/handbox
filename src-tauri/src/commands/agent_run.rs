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

/// 前端事件通道名：每条 AgentEvent 经此发出，payload 为 `{ sessionId, event }`。
const EVENT_NAME: &str = "agent_stream_event";
/// 前端事件通道名：回合终结信号，payload 为 `{ sessionId }`（每个 run 恰好一次）。
const CLOSED_NAME: &str = "agent_stream_closed";

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
    );

    runtime
        .start_run(request.session_id, request.input, sink)
        .await
}
