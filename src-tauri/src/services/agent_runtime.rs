// Agent 模式后端运行时核心。
//
// 负责把一个 Agent Session 的文本对话循环跑起来：装配 hand-agent 的
// `AgentLoopConfig`、seed 历史 transcript、spawn 后台任务驱动
// `run_agent_loop`，并把 `AgentEvent` 逐条转发给一个**事件 sink**。
//
// 本 feature **不含**持久化、错误分型、abort、steering、tools —— 它们是后续
// feature。但事件路径被刻意收敛到**单一 choke point**（`RunSink` + 在
// spawned future 解析后发出的终结事件），以便后续 feature 在同一处接入
// 持久化（on `MessageEnd`）、错误事件映射与 abort 处理。
//
// 并发约束：one-run-per-session —— 同一 `session_id` 已有活跃 run 时，第二个
// `start_run` 以 `AppError` 拒绝，不会启动并发 run。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;

// hand-agent re-exports `CancellationToken`，消费方无需直接依赖 tokio-util。
use hand_agent::{
    run_agent_loop, AgentContext, AgentEvent, AgentEventSink, AgentLoopConfig, CancellationToken,
};
#[cfg(test)]
use hand_ai_model::{self as model};
use hand_ai_model::{Message, UserMessage};

use crate::models::AppError;
use crate::services::chat_engine::{self, ChatOptions};
use crate::services::{AgentSessionService, Database, ProviderService};
use crate::storage::types::UUID;
use crate::storage::AgentSessionRepository;

/// 当前时间（毫秒），用作 transcript 行的 `created_at`。
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 活跃 run 注册表：`session_id -> RunHandle`。
///
/// 用 `Arc<Mutex<..>>` 以便 spawned 后台任务在 loop 结束后能拿到自己的
/// 句柄移除条目（task 是 `'static`，不能借用 `&AgentRuntime`）。
type RunsMap = Arc<Mutex<HashMap<String, RunHandle>>>;

/// 一次活跃 run 的句柄。当前只持有取消 token —— 并发去重需要这张表，且
/// 后续 `agent_run_abort` feature 会从这里取出 token 触发取消。
pub struct RunHandle {
    pub cancel: CancellationToken,
}

/// 事件 sink 抽象 —— 这是后续 feature 的扩展缝。
///
/// run loop 产生的每个 `AgentEvent` 经由 `on_event` 转发为
/// `{ sessionId, event: <AgentEvent JSON> }`；run 结束后（无论 Ok/Err）经由
/// `on_closed` 恰好发出一次终结 payload `{ sessionId }`。
///
/// 命令层注入一个把两者 `window.emit(...)` 出去的 sink；测试注入一个把它们
/// 捕获进 `Vec` 的 sink —— 两条路径走同一套事件语义。
#[derive(Clone)]
pub struct RunSink {
    on_event: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
    on_closed: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
}

impl RunSink {
    /// 构造一个 sink。`on_event` 收到 `{ sessionId, event }`，`on_closed` 收到
    /// 终结 payload `{ sessionId }`。
    pub fn new(
        on_event: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
        on_closed: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
    ) -> Self {
        Self {
            on_event,
            on_closed,
        }
    }
}

/// Agent 模式后端运行时。
pub struct AgentRuntime {
    sessions: AgentSessionService,
    providers: ProviderService,
    /// transcript 增量持久化的写入口。spawn 出去的 run 任务在每个
    /// `AgentEvent::MessageEnd` 上经它把完整 hand-agent `Message` 追加进
    /// `agent_session_messages`（seq 由仓储事务内分配）。
    messages: AgentSessionRepository,
    runs: RunsMap,
    /// 测试专用：注入一个 scripted `StreamFn` 取代真实网络流。生产构造器永远
    /// 把它留空（不启用 model 的 `faux` feature，遵守上游 Decision Log D-04）。
    #[cfg(test)]
    stream_fn_override: Option<hand_agent::StreamFn>,
}

/// `agent_run_stream` 的入参。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    pub session_id: UUID,
    pub input: String,
}

impl AgentRuntime {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            sessions: AgentSessionService::new(Arc::clone(&db)),
            providers: ProviderService::new(Arc::clone(&db)),
            messages: AgentSessionRepository::new(db),
            runs: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            stream_fn_override: None,
        }
    }

    /// 启动一次 run。
    ///
    /// 1. one-run-per-session：若 `runs` 已含 `session_id`，以 `AppError` 拒绝，
    ///    不启动并发 run；否则插入一个新的 `RunHandle`。
    /// 2. 载入 session 行 + 既有 transcript，把每条 `payload` 反序列化为
    ///    hand-agent `Message` 以 seed `AgentContext`，再追加新的 user 消息。
    /// 3. 解析 provider、装配 `model::Model` 与 `SimpleStreamOptions`、构造
    ///    `AgentLoopConfig`（NO tools / NO hooks）。
    /// 4. spawn 后台任务驱动 `run_agent_loop`；每个事件经 sink 转发；loop 返回后
    ///    （任意结果）**恰好一次** 发出终结信号，并从 `runs` 移除 `session_id`。
    pub async fn start_run(
        &self,
        session_id: UUID,
        input: String,
        sink: RunSink,
    ) -> Result<(), AppError> {
        let cancel = CancellationToken::new();

        // --- (1) one-run-per-session：在持锁期间完成检查 + 占位插入 ---
        {
            let mut runs = self.runs.lock().await;
            if runs.contains_key(&session_id) {
                return Err(AppError::with_hint(
                    "AGENT_RUN_ALREADY_ACTIVE",
                    &format!("a run is already active for session: {}", session_id),
                    "请等待当前回合结束后再发送",
                ));
            }
            runs.insert(
                session_id.clone(),
                RunHandle {
                    cancel: cancel.clone(),
                },
            );
        }

        // 从此处起，任何提前返回都必须先把占位从 `runs` 移除，否则会话会被
        // 永久“卡住”。装配阶段的错误经清理后向上抛出。
        match self.assemble_run(&session_id, input, cancel.clone()).await {
            Ok((prompts, context, config)) => {
                self.spawn_loop(session_id, prompts, context, config, cancel, sink);
                Ok(())
            }
            Err(e) => {
                self.runs.lock().await.remove(&session_id);
                Err(e)
            }
        }
    }

    /// 装配 run 所需的 prompts / context / loop config（不触网）。
    ///
    /// 拆成独立步骤是为了让 `start_run` 的占位清理（失败回滚）保持简单，也方便
    /// 单测在不 spawn loop 的前提下断言「解析出的 model id == session 选中的
    /// model id（无静默替换）」。
    async fn assemble_run(
        &self,
        session_id: &UUID,
        input: String,
        cancel: CancellationToken,
    ) -> Result<(Vec<Message>, AgentContext, AgentLoopConfig), AppError> {
        // --- (2) 载入 session 行 ---
        let session = self.sessions.get_session(session_id.clone()).await?;

        let model_id = session
            .model_id
            .clone()
            .ok_or_else(|| AppError::validation_error("agent session has no model_id selected"))?;
        let provider_id = session.provider_id.clone().ok_or_else(|| {
            AppError::validation_error("agent session has no provider_id selected")
        })?;

        // --- (2) seed 既有 transcript：每条 payload -> hand-agent Message ---
        let history = self.sessions.list_messages(session_id.clone()).await?;
        let mut messages: Vec<Message> = Vec::with_capacity(history.len());
        for row in history {
            let msg: Message = serde_json::from_value(row.payload).map_err(|e| {
                AppError::internal_error(&format!(
                    "agent transcript payload (seq {}) is not a valid hand-agent Message: {}",
                    row.seq, e
                ))
            })?;
            messages.push(msg);
        }

        let context = AgentContext {
            system_prompt: session.system_prompt.clone().unwrap_or_default(),
            messages,
        };

        // 新的 user 消息作为本回合的 prompt（run_agent_loop 会把它 push 进
        // context 并发出 message_* 事件）。
        let prompts = vec![Message::User(UserMessage::new_text(input))];

        // --- (3) 解析 provider + 装配 model / stream options / loop config ---
        let provider = self.providers.get_provider(&provider_id).await?;

        let model =
            chat_engine::resolve_model(&provider.provider_type, &model_id, &provider.base_url)?;
        // 显式不静默替换：装配进 loop 的 model id 必须等于 session 选中的 model id。
        debug_assert_eq!(model.id, model_id);

        let chat_options = ChatOptions {
            temperature: session.temperature,
            max_tokens: session.max_tokens.and_then(|t| u32::try_from(t).ok()),
            // thinking_level 直接透传为 reasoning_effort；build_stream_options 内部
            // 用 parse_thinking_level 解析（未知值 -> None，非推理模型不崩）。
            reasoning_effort: session.thinking_level.clone(),
            signal: Some(cancel),
            ..ChatOptions::default()
        };
        let stream_options = chat_engine::build_stream_options(&chat_options, &provider.api_key);

        // NO tools, NO hooks in this feature.
        // `mut` is only exercised by the test-only stream_fn injection below;
        // suppress the unused-mut lint in the production (non-test) build.
        #[cfg_attr(not(test), allow(unused_mut))]
        let mut config = AgentLoopConfig::new(model, stream_options);

        // 测试专用：用 scripted StreamFn 取代真实网络流，使 loop 走确定性路径。
        #[cfg(test)]
        if let Some(stream_fn) = &self.stream_fn_override {
            config.stream_fn = Some(stream_fn.clone());
        }

        Ok((prompts, context, config))
    }

    /// spawn 后台任务驱动 `run_agent_loop`，并保证终结事件恰好发出一次。
    ///
    /// 事件路径的单一 choke point 在此：每个 `AgentEvent` 经 `emit` 转发为
    /// `{ sessionId, event }`；loop future 解析后（任意 Ok/Err）发出终结信号
    /// **恰好一次**，再从 `runs` 移除 `session_id`。后续 feature 把
    /// 错误映射 / abort 接进这同一处。
    ///
    /// 增量持久化（本 feature）也接在这个 choke point 上：每个
    /// `AgentEvent::MessageEnd { message }`（已完成的消息）把完整的 hand-agent
    /// `Message` 序列化后经一个有序的单向 channel 交给一个**串行**持久化任务，
    /// 由它调用 `AgentSessionRepository::append_message` 落库（仓储在同一事务内
    /// 分配 gap-free 的 per-session `seq` 并 bump `message_count`/`last_message_at`）。
    ///
    /// 为什么走 channel 而不在 `emit` 闭包里直接 `append_message`：`emit` 是同步
    /// 的 `Fn`（`AgentEventSink`），不能 `.await`；而 `run_agent_loop` 按事件
    /// 发出顺序同步调用 `emit`，因此把 message 按发出顺序压入 channel、由单一
    /// 消费任务顺序落库，既保住了「user 消息先于 assistant 终结事件落库」的顺序
    /// （user 的 `MessageEnd` 在 assistant streaming 之前发出），又避免阻塞 loop。
    /// 终结信号在 loop 结束**且** channel 排空之后才发出 —— 即每条已完成消息都在
    /// `agent_stream_closed` 之前落库。持久化纯粹是事件处理里的副作用，对发出的
    /// 事件流完全透明（不重排、不重发）。
    fn spawn_loop(
        &self,
        session_id: UUID,
        prompts: Vec<Message>,
        mut context: AgentContext,
        config: AgentLoopConfig,
        cancel: CancellationToken,
        sink: RunSink,
    ) {
        let runs = Arc::clone(&self.runs);

        // 串行持久化管道：emit 闭包（同步）把每个已完成 Message 的
        // (role, payload, created_at) 按发出顺序压入 channel；消费任务按序落库。
        let repo = self.messages.clone();
        let persist_session = session_id.clone();
        let (persist_tx, mut persist_rx) =
            tokio::sync::mpsc::unbounded_channel::<(String, serde_json::Value, i64)>();
        let persist_task = tokio::spawn(async move {
            while let Some((role, payload, created_at)) = persist_rx.recv().await {
                // append_message 失败不影响事件流转发（事件已在 emit 处发出）；
                // 仅记录，避免一条落库失败拖垮整轮持久化。
                if let Err(e) = repo
                    .append_message(&persist_session, &role, &payload, created_at)
                    .await
                {
                    eprintln!(
                        "agent transcript persist failed (session {}): {}",
                        persist_session, e
                    );
                }
            }
        });

        let event_session = session_id.clone();
        let on_event = Arc::clone(&sink.on_event);
        let emit: AgentEventSink = Arc::new(move |event: AgentEvent| {
            // 副作用：仅在 MessageEnd（已完成、可反序列化的完整 Message）上落库；
            // MessageUpdate（streaming delta）绝不落库。run_agent_loop 只为本轮
            // 的新消息发事件，故被 seed 的历史 transcript 不会被重复写入。
            if let AgentEvent::MessageEnd { message } = &event {
                match serde_json::to_value(message) {
                    Ok(payload) => {
                        // role 取序列化后的 tag（Message `#[serde(tag="role")]`），
                        // 与 assemble_run 反序列化 payload 的方式保持一致。
                        let role = payload
                            .get("role")
                            .and_then(|r| r.as_str())
                            .unwrap_or("assistant")
                            .to_string();
                        let _ = persist_tx.send((role, payload, now_ms()));
                    }
                    Err(e) => {
                        eprintln!(
                            "agent transcript serialize failed (session {}): {}",
                            event_session, e
                        );
                    }
                }
            }

            let event_json = match serde_json::to_value(&event) {
                Ok(v) => v,
                Err(e) => json!({ "type": "serializeError", "message": e.to_string() }),
            };
            on_event(json!({
                "sessionId": event_session,
                "event": event_json,
            }));
        });

        let on_closed = Arc::clone(&sink.on_closed);
        let closed_session = session_id;

        // 镜像 `commands/message.rs` 的后台流式模式：spawn 后立即返回，事件经
        // sink 异步抵达。用 `tokio::spawn` 与 `message_user_send_stream` 一致。
        tokio::spawn(async move {
            // 驱动 loop。`cancel` 与 `config.stream_options.base.signal` 是同一个
            // token（见 start_run），也是 `runs` 中登记的那一个 —— abort feature
            // 会从注册表取出它触发取消。任意结果（Ok 自然完成 / Err）都走同一个
            // 终结路径。
            let _ = run_agent_loop(
                prompts,
                &mut context,
                &[],
                &config,
                chat_engine::shared_client(),
                &emit,
                &cancel,
            )
            .await;

            // loop 结束后丢弃 emit（其中持有 persist_tx），关闭 channel；等持久化
            // 任务把已入队的消息全部落库后再发终结信号 —— 保证每条已完成消息都在
            // `agent_stream_closed` 之前入库。
            drop(emit);
            let _ = persist_task.await;

            // --- (4) 终结：恰好一次发出 closed，再移除注册表条目 ---
            on_closed(json!({ "sessionId": closed_session }));
            runs.lock().await.remove(&closed_session);
        });
    }

    /// 当前活跃 run 数量（测试辅助）。
    #[cfg(test)]
    async fn active_run_count(&self) -> usize {
        self.runs.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::{AgentSession, Provider};
    use crate::storage::{AgentSessionRepository, ProviderRepository};
    use hand_ai_model::{AssistantMessage, AssistantMessageEvent, StopReason, Usage};
    use sqlx::Row;
    use std::sync::Mutex as StdMutex;
    use tempfile::TempDir;

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    async fn test_db() -> (Arc<Database>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.unwrap());
        (db, temp_dir)
    }

    /// Seed a catalog provider (`openai`) so `resolve_model` succeeds offline.
    async fn seed_provider(db: &Arc<Database>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let provider = Provider {
            id: id.clone(),
            name: "Test OpenAI".to_string(),
            provider_type: "openai".to_string(),
            base_url: String::new(),
            api_key: "sk-test".to_string(),
            enabled: true,
            created_at: now_ms(),
            updated_at: now_ms(),
        };
        ProviderRepository::new(Arc::clone(db))
            .create_provider(&provider)
            .await
            .unwrap();
        id
    }

    /// Seed an agent session selecting `gpt-4o` under the given provider.
    async fn seed_session(db: &Arc<Database>, provider_id: &str, model_id: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let session = AgentSession {
            id: id.clone(),
            name: "Run Session".to_string(),
            model_id: Some(model_id.to_string()),
            provider_id: Some(provider_id.to_string()),
            system_prompt: Some("You are a helpful agent.".to_string()),
            thinking_level: None,
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir: None,
            enabled_tools: vec![],
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            created_at: now_ms(),
            updated_at: now_ms(),
        };
        AgentSessionRepository::new(Arc::clone(db))
            .create_session(&session)
            .await
            .unwrap();
        id
    }

    /// Build a finished `AssistantMessage` for a scripted `Done` event.
    fn done_message(model_id: &str, text: &str) -> AssistantMessage {
        AssistantMessage {
            role: "assistant".into(),
            content: vec![model::AssistantContentBlock::Text(model::TextContent::new(
                text.to_string(),
            ))],
            api: model::Api::OpenAICompletions,
            provider: model::types::Provider::OpenAI,
            model: model_id.to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        }
    }

    /// A capturing sink: records every event payload and every closed payload.
    #[derive(Clone, Default)]
    struct CapturingSink {
        events: Arc<StdMutex<Vec<serde_json::Value>>>,
        closed: Arc<StdMutex<Vec<serde_json::Value>>>,
    }

    impl CapturingSink {
        fn into_run_sink(self) -> RunSink {
            let events = Arc::clone(&self.events);
            let closed = Arc::clone(&self.closed);
            RunSink::new(
                Arc::new(move |v| events.lock().unwrap().push(v)),
                Arc::new(move |v| closed.lock().unwrap().push(v)),
            )
        }

        fn closed_count(&self) -> usize {
            self.closed.lock().unwrap().len()
        }

        fn event_count(&self) -> usize {
            self.events.lock().unwrap().len()
        }
    }

    /// Poll until the sink has observed the terminal closed signal (or panic on
    /// timeout). Deterministic because the scripted stream completes promptly.
    async fn wait_for_closed(sink: &CapturingSink) {
        for _ in 0..200 {
            if sink.closed_count() >= 1 {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("run did not close within timeout");
    }

    /// VAL-RUN-015 + assertion (2): a scripted run emits >=1 event and EXACTLY
    /// ONE closed signal, and the model resolved into the loop equals the
    /// session's selected model id (no silent substitution).
    #[tokio::test]
    async fn scripted_run_emits_events_and_exactly_one_closed() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // Capture the model id that reaches the loop boundary (proves resolution
        // without persistence).
        let seen_model: Arc<StdMutex<Option<String>>> = Arc::new(StdMutex::new(None));
        let seen_model_cl = Arc::clone(&seen_model);

        let stream_fn: hand_agent::StreamFn = Arc::new(move |model, _ctx, _opts, _cancel| {
            *seen_model_cl.lock().unwrap() = Some(model.id.clone());
            let done = AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: done_message(&model.id, "hi there"),
            };
            let events = vec![
                AssistantMessageEvent::Start {
                    partial: done_message(&model.id, ""),
                },
                AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "hi there".into(),
                    partial: done_message(&model.id, "hi there"),
                },
                done,
            ];
            Box::pin(futures::stream::iter(events))
        });

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(stream_fn);

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hello".to_string(),
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        // EXACTLY ONE closed signal for the session.
        assert_eq!(sink.closed_count(), 1, "exactly one agent_stream_closed");
        // At least one event was emitted (agent_start / message_* / done ...).
        assert!(sink.event_count() >= 1, "run must emit >= 1 event");

        // The model resolved into the loop equals the session's selection.
        assert_eq!(
            seen_model.lock().unwrap().as_deref(),
            Some("gpt-4o"),
            "no silent model substitution at the loop boundary"
        );

        // The run drained from the registry after closing.
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-RUN-009: a second `start_run` for a session with an active run does
    /// NOT start a concurrent run — it is rejected. The `runs` map holds one
    /// entry while the first run is in flight, then empties.
    #[tokio::test]
    async fn second_concurrent_run_is_rejected_single_run_per_session() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // Gate the scripted stream on a release signal so the first run stays
        // active long enough to attempt a second concurrent run.
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let release_rx = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));

        let stream_fn: hand_agent::StreamFn = Arc::new(move |model, _ctx, _opts, _cancel| {
            let release_rx = Arc::clone(&release_rx);
            let model_id = model.id.clone();
            Box::pin(futures::stream::once(async move {
                if let Some(rx) = release_rx.lock().await.take() {
                    let _ = rx.await;
                }
                AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_message(&model_id, "done"),
                }
            }))
        });

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(stream_fn);

        let first_sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "first".to_string(),
                first_sink.clone().into_run_sink(),
            )
            .await
            .expect("first run starts");

        // First run is in flight (stream gated): the registry holds one entry.
        assert_eq!(runtime.active_run_count().await, 1);

        // Second run on the same session must be rejected (no concurrent run).
        let second_sink = CapturingSink::default();
        let err = runtime
            .start_run(
                session_id.clone(),
                "second".to_string(),
                second_sink.clone().into_run_sink(),
            )
            .await
            .expect_err("second concurrent run must be rejected");
        assert_eq!(err.code, "AGENT_RUN_ALREADY_ACTIVE");

        // Still exactly one active run; the rejected run emitted nothing.
        assert_eq!(runtime.active_run_count().await, 1);
        assert_eq!(second_sink.closed_count(), 0);
        assert_eq!(second_sink.event_count(), 0);

        // Release the first run; it completes and the registry empties.
        let _ = release_tx.send(());
        wait_for_closed(&first_sink).await;
        assert_eq!(first_sink.closed_count(), 1);

        for _ in 0..200 {
            if runtime.active_run_count().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            runtime.active_run_count().await,
            0,
            "registry empties after the single run completes"
        );
    }

    /// A persisted transcript row, projected from the DB via SQL so the tests
    /// can assert directly on the stored `role` / `seq` / `payload` columns and
    /// on SQLite's own `json_valid(payload)` verdict.
    #[derive(Debug)]
    struct StoredRow {
        seq: i64,
        role: String,
        json_valid: i64,
        payload_model: Option<String>,
    }

    /// Read the persisted transcript rows for a session, ordered by `seq`,
    /// asking SQLite to validate each payload (`json_valid`) and to extract the
    /// payload's `$.model` field (NULL for non-assistant rows).
    async fn query_messages(db: &Arc<Database>, session_id: &str) -> Vec<StoredRow> {
        let rows = sqlx::query(
            r#"
            SELECT seq,
                   role,
                   json_valid(payload) AS json_valid,
                   json_extract(payload, '$.model') AS payload_model
            FROM agent_session_messages
            WHERE session_id = $1
            ORDER BY seq ASC
        "#,
        )
        .bind(session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();

        rows.into_iter()
            .map(|row| StoredRow {
                seq: row.try_get("seq").unwrap(),
                role: row.try_get("role").unwrap(),
                json_valid: row.try_get("json_valid").unwrap(),
                payload_model: row.try_get("payload_model").ok().flatten(),
            })
            .collect()
    }

    /// A single-turn scripted stream: Start -> TextDelta -> Done. Records the
    /// model id that reaches the loop boundary so tests can also confirm the
    /// assembled model matches the session selection.
    fn single_turn_stream_fn(text: &str) -> hand_agent::StreamFn {
        let text = text.to_string();
        Arc::new(move |model, _ctx, _opts, _cancel| {
            let events = vec![
                AssistantMessageEvent::Start {
                    partial: done_message(&model.id, ""),
                },
                AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: text.clone(),
                    partial: done_message(&model.id, &text),
                },
                AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_message(&model.id, &text),
                },
            ];
            Box::pin(futures::stream::iter(events))
        })
    }

    /// VAL-PERSIST-001 + VAL-PERSIST-002 + VAL-RUN-004: a scripted run persists
    /// the new transcript incrementally — a user row AND an assistant row land
    /// in `agent_session_messages`; EVERY row's payload is valid JSON; the user
    /// row precedes the assistant row by `seq` (user MessageEnd persists before
    /// the assistant MessageEnd); and the assistant payload's `model` field
    /// equals the session's selected model id (no silent substitution).
    #[tokio::test]
    async fn run_persists_user_and_assistant_rows_with_valid_json() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(single_turn_stream_fn("hi there"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hello".to_string(),
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        let rows = query_messages(&db, &session_id).await;

        // Exactly the two NEW messages of this run: user prompt + assistant.
        // (No seeded prior transcript, so nothing should be re-persisted.)
        assert_eq!(rows.len(), 2, "user row + assistant row persisted");

        // VAL-PERSIST-001: every persisted row's payload is valid JSON.
        for row in &rows {
            assert_eq!(
                row.json_valid, 1,
                "row seq {} payload must be valid JSON",
                row.seq
            );
        }

        // VAL-PERSIST-002: user is persisted before the assistant terminal —
        // proven by ordering: seq(user) < seq(assistant), gap-free from 0.
        assert_eq!(
            rows[0].role, "user",
            "first persisted row is the user message"
        );
        assert_eq!(rows[0].seq, 0);
        assert_eq!(
            rows[1].role, "assistant",
            "second persisted row is the assistant message"
        );
        assert_eq!(rows[1].seq, 1);

        // VAL-RUN-004: the persisted assistant payload's model == session model.
        assert_eq!(
            rows[1].payload_model.as_deref(),
            Some("gpt-4o"),
            "assistant payload.model equals the selected model id (no silent substitution)"
        );

        // Session counters reflect the two appends (bumped transactionally).
        let session = runtime
            .sessions
            .get_session(session_id.clone())
            .await
            .unwrap();
        assert_eq!(session.message_count, 2);

        // Persistence drained before close (incremental): the run is gone from
        // the registry and exactly one closed signal fired.
        assert_eq!(sink.closed_count(), 1);
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-PERSIST-002 (ordering proof, isolated): regardless of payload shape,
    /// the user message always lands at seq 0 (its `MessageEnd` fires at the
    /// very start of the loop, before any assistant streaming), so
    /// seq(user) < seq(assistant) holds for the persisted rows.
    #[tokio::test]
    async fn user_message_persisted_before_assistant_terminal() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(single_turn_stream_fn("response"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "the user prompt".to_string(),
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        let rows = query_messages(&db, &session_id).await;
        let user_seq = rows
            .iter()
            .find(|r| r.role == "user")
            .map(|r| r.seq)
            .expect("a user row must be persisted");
        let assistant_seq = rows
            .iter()
            .find(|r| r.role == "assistant")
            .map(|r| r.seq)
            .expect("an assistant row must be persisted");

        assert!(
            user_seq < assistant_seq,
            "user message (seq {}) must persist before the assistant terminal (seq {})",
            user_seq,
            assistant_seq
        );
    }

    /// VAL-PERSIST-003: completed messages land incrementally (each at its
    /// MessageEnd, before the run's `agent_stream_closed`). This run reaches two
    /// completed MessageEnds (user + assistant) and then the scripted stream
    /// ends; both already-completed rows remain persisted. The persistence
    /// logic is abort-agnostic — it only reacts to MessageEnd — so any message
    /// that reached MessageEnd before an (out-of-scope) abort is already stored.
    #[tokio::test]
    async fn completed_messages_persist_incrementally_and_survive_stream_end() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(single_turn_stream_fn("completed turn"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "go".to_string(),
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        // Both completed messages survived the stream end, persisted before the
        // closed signal (drain-before-close), with gap-free seqs from 0.
        let rows = query_messages(&db, &session_id).await;
        assert_eq!(rows.len(), 2, "both completed messages are persisted");
        let seqs: Vec<i64> = rows.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![0, 1], "completed rows keep gap-free seqs");
        assert_eq!(sink.closed_count(), 1, "exactly one closed after drain");
    }

    /// The seeded prior transcript is NOT re-persisted: only the NEW messages of
    /// this run are appended on top of the existing history. After a run on a
    /// session that already holds 2 transcript rows, the table holds 2 prior +
    /// 2 new = 4 rows, with the new rows appended after the seeded seqs.
    #[tokio::test]
    async fn seeded_prior_transcript_is_not_repersisted() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // Seed two prior transcript rows (as if from earlier runs).
        let repo = AgentSessionRepository::new(Arc::clone(&db));
        let prior_user = serde_json::to_value(Message::User(UserMessage::new_text(
            "earlier question".to_string(),
        )))
        .unwrap();
        let prior_assistant =
            serde_json::to_value(Message::Assistant(done_message("gpt-4o", "earlier answer")))
                .unwrap();
        repo.append_message(&session_id, "user", &prior_user, now_ms())
            .await
            .unwrap();
        repo.append_message(&session_id, "assistant", &prior_assistant, now_ms())
            .await
            .unwrap();

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(single_turn_stream_fn("new answer"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "new question".to_string(),
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        let rows = query_messages(&db, &session_id).await;
        // 2 prior (seqs 0,1) + 2 new (seqs 2,3); seeded rows not duplicated.
        assert_eq!(rows.len(), 4, "prior 2 + new 2, no re-persistence of seed");
        let seqs: Vec<i64> = rows.iter().map(|r| r.seq).collect();
        assert_eq!(
            seqs,
            vec![0, 1, 2, 3],
            "new rows appended after seeded seqs"
        );
        assert_eq!(rows[2].role, "user", "new user row appended at seq 2");
        assert_eq!(
            rows[3].role, "assistant",
            "new assistant row appended at seq 3"
        );
    }
}
