// Agent 模式后端运行时核心。
//
// 负责把一个 Agent Session 的文本对话循环跑起来：装配 hand-agent 的
// `AgentLoopConfig`、seed 历史 transcript、spawn 后台任务驱动
// `run_agent_loop`，并把 `AgentEvent` 逐条转发给一个**事件 sink**。
//
// 本 feature 含持久化（on `MessageEnd`）与**错误分型/不泄密**；abort、steering、
// tools 仍是后续 feature。事件路径被刻意收敛到**单一 choke point**（`RunSink` +
// 在 spawned future 解析后发出的终结事件），错误事件映射也接在这同一处。
//
// 错误有两个面（经 hand-ai 源码核对，见 D17）：
//
// 1. **run-level `Err`（envelope）**：`run_agent_loop` 返回 `Err(AgentError)`
//    （例如底层 `client.stream_simple` 返回 `ProviderNotFound`、或模型解析失败）。
//    runtime 捕获该 `Err`、把它映射为一个 **sanitized** 的 `{code,message,hint}`，
//    在终结的 `agent_stream_closed` **之前**经 `on_error` 发出一个
//    `agent_stream_error { sessionId, error }`；此路径不产出任何 assistant 内容。
//
// 2. **in-band error**：流以 `AssistantMessageEvent::Error` 收尾，hand-agent 把它
//    转成一条 `stop_reason == Error` 的终结 assistant `Message` 并照常发出
//    `MessageEnd`（因此被正常持久化）。这条**不需要**也**不发** envelope —— 它
//    走与成功回合相同的事件/持久化路径，只是 `stopReason=error`（含
//    `errorMessage`）。流中途断网也归入此路径：断网前已 `MessageEnd` 的内容已落库。
//
// SECURITY（D24/D14）：envelope 的 `{code,message,hint}` 永不回显可能携带 API key
// 或带凭据 URL 的原始 provider 错误文本；只映射为稳定 `code`（AppError 词汇表）+
// 通用但有用的 message。
//
// closed-once 不变量是神圣的：无论 run 以 Ok / Err(envelope) / in-band-error 收尾，
// `agent_stream_closed` 都**恰好发出一次**，且只有一个 closed emit site。
//
// 并发约束：one-run-per-session —— 同一 `session_id` 已有活跃 run 时，第二个
// `start_run` 以 `AppError` 拒绝，不会启动并发 run。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde_json::json;
use tokio::sync::Mutex;

// hand-agent re-exports `CancellationToken`，消费方无需直接依赖 tokio-util。
use hand_agent::{
    run_agent_loop, AgentContext, AgentError, AgentEvent, AgentEventSink, AgentLoopConfig,
    AgentTool, CancellationToken, ToolExecutionMode,
};
#[cfg(test)]
use hand_ai_model::{self as model};
use hand_ai_model::{ImageContent, Message, TextContent, UserContentBlock, UserMessage};

use crate::models::AppError;
use crate::services::agent_tools;
use crate::services::chat_engine::{self, ChatOptions};
use crate::services::skill_service::SkillService;
use crate::services::{
    AgentSessionService, Database, ProviderService, SettingsService, StorageService,
};
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

/// 一次活跃 run 的句柄。持有取消 token（`agent_run_abort` 从这里取出 token 触发
/// 取消）与 steering 队列。
///
/// `steering` 是本 run 的待注入 user 消息队列：`AgentRuntime::steer` 经注册表把
/// 消息 push 进来；run loop 经 `get_steering_messages` 闭包在每个 turn 边界**drain**
/// 它（见 `assemble_run`）。二者共享**同一个** `Arc<Mutex<..>>` —— 这是 steering
/// 抵达进行中 run 的关键：命令通过 handle 入队，loop 通过闭包出队。
pub struct RunHandle {
    pub cancel: CancellationToken,
    /// 本 run 的 steering 队列（与 `get_steering_messages` 闭包共享同一 `Arc`）。
    pub steering: Arc<Mutex<Vec<Message>>>,
}

/// 事件 sink 抽象 —— 这是错误分型与后续 feature 的扩展缝。
///
/// run loop 产生的每个 `AgentEvent` 经由 `on_event` 转发为
/// `{ sessionId, event: <AgentEvent JSON> }`；run 结束后（无论 Ok/Err）经由
/// `on_closed` 恰好发出一次终结 payload `{ sessionId }`。
///
/// run-level `Err`（envelope）经由可选的 `on_error` 转发为
/// `{ sessionId, error: { code, message, hint } }`（sanitized），在 `on_closed`
/// **之前**发出。`on_error` 是可选的：未注入时（旧调用方），envelope 退回经
/// `on_event` 发出，仍保证错误抵达 UI 而不引入第二个 closed emit site。
///
/// 命令层注入一个把它们 `window.emit(...)` 出去的 sink；测试注入一个把它们
/// 捕获进 `Vec` 的 sink —— 两条路径走同一套事件语义。
#[derive(Clone)]
pub struct RunSink {
    on_event: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
    on_closed: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
    /// run-level `Err` envelope 的专用通道。`None` 时 envelope 退回经 `on_event`。
    on_error: Option<Arc<dyn Fn(serde_json::Value) + Send + Sync>>,
}

impl RunSink {
    /// 构造一个 sink。`on_event` 收到 `{ sessionId, event }`，`on_closed` 收到
    /// 终结 payload `{ sessionId }`。envelope 默认退回 `on_event`（见
    /// [`RunSink::with_error`] 注入专用通道）。
    pub fn new(
        on_event: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
        on_closed: Arc<dyn Fn(serde_json::Value) + Send + Sync>,
    ) -> Self {
        Self {
            on_event,
            on_closed,
            on_error: None,
        }
    }

    /// 注入 run-level `Err` envelope 的专用通道，得到一个会把
    /// `{ sessionId, error }` 发给 `on_error`（而非 `on_event`）的 sink。
    pub fn with_error(mut self, on_error: Arc<dyn Fn(serde_json::Value) + Send + Sync>) -> Self {
        self.on_error = Some(on_error);
        self
    }
}

/// 把一个 run-level `AgentError` 映射成一个 **sanitized** 的 `AppError`
/// `{ code, message, hint }`，供 `agent_stream_error` envelope 使用。
///
/// SECURITY（D24/D14）：**绝不** 把原始 provider/transport 错误文本逐字回显进
/// `message` —— 它可能携带 API key 或带凭据的请求 URL。每个变体映射到一个稳定的
/// AppError code（AUTH_ERROR / NETWORK_ERROR / RATE_LIMIT / INTERNAL_ERROR）加上
/// 一句通用但有用的提示。仅 `ClientError::ProviderNotFound` 的 `model_id`（来自
/// 我们自己的 session 选择，非密文）被引用以便定位配置问题。
fn sanitize_agent_error(err: &AgentError) -> AppError {
    use hand_ai_model::ClientError;

    match err {
        // 底层 client 错误：provider 未注册 / 缺凭据 / 流空。统一归为配置/认证类，
        // 不回显底层 Display（其中可能含 api 标识但我们只取已知安全的字段）。
        AgentError::Client(client_err) => match client_err {
            ClientError::ProviderNotFound { model_id, .. } => AppError::with_hint(
                "AUTH_ERROR",
                &format!("no provider is configured for model \"{}\"", model_id),
                "请在设置中为该模型配置可用的供应商与 API Key",
            ),
            ClientError::OAuthRequired { .. } => {
                AppError::auth_error("the selected provider requires sign-in credentials")
            }
            ClientError::StreamEndedWithoutResult => {
                AppError::network_error("the model stream ended without producing a response")
            }
        },
        // HTTP transport / proxy 失败：按 status 粗分认证 / 限流 / 网络，
        // **不**回显 `message`（可能含 URL 或上游回包细节）。
        AgentError::Proxy { status, .. } => match status {
            401 | 403 => {
                AppError::auth_error("the provider rejected the request (authentication failed)")
            }
            429 => AppError::rate_limit_error(),
            _ => AppError::network_error("the provider request failed"),
        },
        // 取消属于 abort（后续 feature 的正常路径），但若以 Err 形式到达，
        // 仍给出非泄密的通用说明。
        AgentError::Aborted => {
            AppError::with_hint("INTERNAL_ERROR", "the run was aborted", "请重试该回合")
        }
        // 其余（InvalidState / SchemaValidation / Other）属于内部/装配类问题。
        // 这些变体的文本来自我们自己的代码，不含 provider 密文，但仍走通用 code。
        _ => AppError::internal_error("the agent run failed to complete"),
    }
}

/// Map a session's `tool_execution_mode` string to hand-agent's
/// [`ToolExecutionMode`]. `"sequential"` (case-insensitive) maps to
/// `Sequential`; every other value — including `None`/unset and unknown
/// strings — falls back to `Parallel`, matching hand-agent's own default.
fn tool_execution_mode_from(raw: Option<&str>) -> ToolExecutionMode {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("sequential") => ToolExecutionMode::Sequential,
        _ => ToolExecutionMode::Parallel,
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
    /// Skill discovery facade (M2 wiring). Resolves the three scope roots and
    /// runs discovery at assemble time so the run's enabled skills can be turned
    /// into a system-prompt index plus an auto-injected `skill` tool.
    skill_service: Arc<SkillService>,
    /// Settings store (config.json). `assemble_run` reads `skills.disabled`
    /// FRESH from it on every run — never cached — so toggling a skill's
    /// global opt-out applies to the very next run without a restart.
    settings: SettingsService,
    /// 测试专用：注入一个 scripted `StreamFn` 取代真实网络流。生产构造器永远
    /// 把它留空（不启用 model 的 `faux` feature，遵守上游 Decision Log D-04）。
    #[cfg(test)]
    stream_fn_override: Option<hand_agent::StreamFn>,
    /// 测试专用：用一个指定的 `model::Model` 取代 `resolve_model` 的结果。
    ///
    /// 用途：注入一个 `api` 在 `shared_client()`（builtins-only）中**未注册**的
    /// model（例如 `Api::Faux`），使真实的 `client.stream_simple` 返回
    /// `ProviderNotFound` -> `run_agent_loop` 返回真实的 `Err(AgentError::Client)`，
    /// 从而以**真实路径**覆盖 run-level `Err`（envelope）的映射 + 不泄密。
    /// 这不启用 `faux` Cargo feature（`Api::Faux` 是核心 enum 变体）。
    #[cfg(test)]
    model_override: Option<hand_ai_model::Model>,
}

/// 一个随本回合输入一并发送的图片附件（镜像 chat 的 `MessageRequestAttachment`）。
///
/// 前端已把文件读成原始字节并按 `image/*` 过滤；后端在装配 user 消息时把每个
/// 图片字节做 base64 STANDARD 编码，emit 成一个 `model::ImageContent` 块。非图片
/// mime 在 `build_user_message` 里被防御性跳过（belt-and-suspenders）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// `agent_run_stream` 的入参。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunRequest {
    pub session_id: UUID,
    pub input: String,
    /// 可选的图片附件。缺省（旧调用方 / 纯文本发送）时为空 Vec，走纯文本路径。
    #[serde(default)]
    pub attachments: Vec<AgentRunAttachment>,
}

/// Per-image byte cap enforced at the IPC boundary (CLAUDE.md「输入验证必须完备」)。
/// 前端已把附件限制在 10 MiB，但后端不信任前端：超界的附件被防御性跳过，绝不把无界
/// 字节 base64 进模型上下文。
const ATTACHMENT_BYTE_CAP: usize = 10 * 1024 * 1024;
/// 每回合接受的附件数量上限。超过该计数的尾部附件被防御性跳过，使一个病态请求无法
/// 把装配出的消息撑爆。
const ATTACHMENT_MAX_COUNT: usize = 16;

/// 从本回合输入与（可选的）图片附件装配一条 user `Message`。
///
/// - 无附件（或全部被防御性跳过）时，回退为纯文本 `UserMessage::new_text(input)`，
///   与现有路径完全一致。
/// - 有图片附件时，emit 一条 **content-block** `UserMessage`：每个 `image/*` 附件的
///   字节做 base64 STANDARD 编码后成为一个 `UserContentBlock::Image(ImageContent)`，
///   顺序为**图片块在前、文本块在后**（镜像 `chat_engine::chat_user_message`）。
/// - 仅 `image/*` mime 成为图片块；非图片附件被防御性跳过（前端已过滤，这里是
///   belt-and-suspenders）。文本为空时不追加文本块。
/// - **IPC 边界校验**：超过 `ATTACHMENT_BYTE_CAP` 的单图被跳过；附件数量超过
///   `ATTACHMENT_MAX_COUNT` 的尾部被截断 —— 前端虽已限制，但 IPC 边界必须独立校验
///   （CLAUDE.md「输入验证必须完备」），不信任前端。
///
/// 抽成独立可测函数：使「图片附件 -> ImageContent 块」的装配可在不 spawn loop、
/// 不触网的前提下单测（VAL-RUN-018）。
fn build_user_message(input: String, attachments: &[AgentRunAttachment]) -> UserMessage {
    if attachments.is_empty() {
        return UserMessage::new_text(input);
    }

    let mut blocks: Vec<UserContentBlock> = Vec::with_capacity(attachments.len() + 1);
    // 在 IPC 边界限制数量：只考虑前 ATTACHMENT_MAX_COUNT 个附件，其余防御性跳过。
    for att in attachments.iter().take(ATTACHMENT_MAX_COUNT) {
        if !att.mime_type.starts_with("image/") {
            // 非图片附件防御性跳过（前端已按 image/* 过滤）。
            continue;
        }
        if att.data.len() > ATTACHMENT_BYTE_CAP {
            // 超出单图字节上限：防御性跳过，不把无界字节 base64 进上下文。
            continue;
        }
        let data_b64 = BASE64_STANDARD.encode(&att.data);
        blocks.push(UserContentBlock::Image(ImageContent::new(
            data_b64,
            att.mime_type.clone(),
        )));
    }

    if blocks.is_empty() {
        // 全部附件被跳过 —— 回退为纯文本。
        return UserMessage::new_text(input);
    }

    if !input.is_empty() {
        blocks.push(UserContentBlock::Text(TextContent::new(input)));
    }
    UserMessage::new_blocks(blocks)
}

impl AgentRuntime {
    /// Construct a runtime with an inert skill service (no skills discovered)
    /// and an inert settings store (default settings, empty disabled list).
    ///
    /// Retained as the dependency-light constructor used by tests and by call
    /// sites that do not resolve OS skill roots. Production wires the real
    /// skill service and the shared `SettingsService` via
    /// [`AgentRuntime::new_with_skills`] (the production caller holds the
    /// `AppHandle` needed to resolve the app-data and home directories).
    pub fn new(db: Arc<Database>) -> Self {
        // Roots that do not exist → discovery silently finds nothing (missing
        // roots are skipped, not errors), so this runtime never injects skills.
        let inert_skills = Arc::new(SkillService::new(
            PathBuf::from("/nonexistent/handbox-skills/appdata"),
            PathBuf::from("/nonexistent/handbox-skills/user"),
        ));
        // A settings store rooted under a unique temp dir: `get_settings()`
        // materializes a default config there (empty `skills.disabled`), so
        // this runtime applies no global opt-out — matching its inert skills.
        let settings_dir = std::env::temp_dir()
            .join("handbox-inert-settings")
            .join(uuid::Uuid::new_v4().to_string());
        let storage =
            StorageService::new(settings_dir).expect("create inert settings dir under temp_dir");
        let inert_settings = SettingsService::new(Arc::new(storage));
        Self::new_with_skills(db, inert_skills, inert_settings)
    }

    /// Construct a runtime with an explicit skill service and settings store.
    /// The production path (`initialize_services`) passes a skill service whose
    /// roots are resolved from Tauri's `PathResolver` (`<app_data_dir>/skills`
    /// and `~/.agents/skills`) and a clone of the managed `SettingsService`
    /// (the global skill opt-out list lives in its config.json).
    pub fn new_with_skills(
        db: Arc<Database>,
        skill_service: Arc<SkillService>,
        settings: SettingsService,
    ) -> Self {
        Self {
            sessions: AgentSessionService::new(Arc::clone(&db)),
            providers: ProviderService::new(Arc::clone(&db)),
            messages: AgentSessionRepository::new(db),
            runs: Arc::new(Mutex::new(HashMap::new())),
            skill_service,
            settings,
            #[cfg(test)]
            stream_fn_override: None,
            #[cfg(test)]
            model_override: None,
        }
    }

    /// 启动一次 run。
    ///
    /// 1. one-run-per-session：若 `runs` 已含 `session_id`，以 `AppError` 拒绝，
    ///    不启动并发 run；否则插入一个新的 `RunHandle`。
    /// 2. 载入 session 行 + 既有 transcript，把每条 `payload` 反序列化为
    ///    hand-agent `Message` 以 seed `AgentContext`，再追加新的 user 消息。
    /// 3. 解析 provider、装配 `model::Model` 与 `SimpleStreamOptions`、构造
    ///    `AgentLoopConfig`（含 per-session 内置工具集 + `tool_execution` 模式，
    ///    NO hooks）。
    /// 4. spawn 后台任务驱动 `run_agent_loop`；每个事件经 sink 转发；loop 返回后
    ///    （任意结果）**恰好一次** 发出终结信号，并从 `runs` 移除 `session_id`。
    pub async fn start_run(
        &self,
        session_id: UUID,
        input: String,
        attachments: Vec<AgentRunAttachment>,
        sink: RunSink,
    ) -> Result<(), AppError> {
        let cancel = CancellationToken::new();
        // 本 run 的 steering 队列。这个 `Arc` 同时被登记进 `RunHandle`（命令经
        // 它入队）与 `get_steering_messages` 闭包（loop 经它 drain）—— 同一个
        // `Arc` 是 steering 抵达进行中 run 的关键。在占位插入处就创建，使其从 run
        // 登记的那一刻起即可用（与 cancel token 同一时点）。
        let steering: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));

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
                    steering: Arc::clone(&steering),
                },
            );
        }

        // 从此处起，任何提前返回都必须先把占位从 `runs` 移除，否则会话会被
        // 永久“卡住”。装配阶段的错误经清理后向上抛出。
        match self
            .assemble_run(
                &session_id,
                input,
                attachments,
                cancel.clone(),
                Arc::clone(&steering),
            )
            .await
        {
            Ok((prompts, context, config, tools)) => {
                self.spawn_loop(session_id, prompts, context, config, tools, cancel, sink);
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
        attachments: Vec<AgentRunAttachment>,
        cancel: CancellationToken,
        steering: Arc<Mutex<Vec<Message>>>,
    ) -> Result<(Vec<Message>, AgentContext, AgentLoopConfig, Vec<AgentTool>), AppError> {
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

        let mut context = AgentContext {
            system_prompt: session.system_prompt.clone().unwrap_or_default(),
            messages,
        };

        // 新的 user 消息作为本回合的 prompt（run_agent_loop 会把它 push 进
        // context 并发出 message_* 事件）。含图片附件时装配成 content-block
        // UserMessage（base64 -> ImageContent，图片块在前）；否则走纯文本路径。
        let prompts = vec![Message::User(build_user_message(input, &attachments))];

        // --- (3) 解析 provider + 装配 model / stream options / loop config ---
        let provider = self.providers.get_provider(&provider_id).await?;

        #[cfg_attr(not(test), allow(unused_mut))]
        let mut model =
            chat_engine::resolve_model(&provider.provider_type, &model_id, &provider.base_url)?;
        // 显式不静默替换：装配进 loop 的 model id 必须等于 session 选中的 model id。
        debug_assert_eq!(model.id, model_id);

        // 测试专用：用一个 API 未注册的 model 取代解析结果，以走真实
        // `ProviderNotFound` -> `Err(AgentError)` 的 envelope 路径（见字段注释）。
        #[cfg(test)]
        if let Some(override_model) = &self.model_override {
            model = override_model.clone();
        }

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

        // Per-session built-in tool set. `build_tools` only registers the
        // session's enabled, supported, read-only tools: a tool toggled OFF is
        // absent, the FS tools are omitted when `working_dir` is empty/None, and
        // mutating names (`write_file`/`run_command`) are never produced. The
        // gate is therefore "what is not registered cannot run" — the loop's own
        // `Tool '<name>' not found` enforces it (D8/D9). No hooks in this feature.
        let working_dir = session.working_dir.as_deref().map(Path::new);
        let mut tools = agent_tools::build_tools(&session.enabled_tools, working_dir);

        // Skill wiring: discover skills across the three scope roots, then take
        // the run's EFFECTIVE set = discovered-and-validated ∖ the GLOBAL
        // opt-out list (settings `skills.disabled`). There is NO per-session
        // filter: a freshly dropped valid skill is usable on the next run with
        // zero enablement action (default all-on, VAL-CROSS-004). Only when the
        // effective set is non-empty: (a) append the available-skills index to
        // the system prompt and (b) auto-inject the `skill` tool (NOT via
        // enabled_tools). Discovery errors are non-fatal: they are logged but
        // never abort the run.
        let (discovered, skill_errs) = self.skill_service.discover(working_dir);
        if !skill_errs.is_empty() {
            tracing::warn!(
                "skill discovery produced {} non-fatal diagnostic(s)",
                skill_errs.len()
            );
        }
        // The disabled list is read FRESH from config.json on every run — never
        // cached — so a toggle applies to the very next run without a restart
        // (VAL-ASSEMBLE-005). A settings read failure aborts assembly with its
        // structured error rather than failing open (failing open would
        // silently re-enable skills the user disabled). Matching is exact: the
        // list is opaque storage (no trim, no case folding), so orphan or
        // whitespace entries simply never match a discovered name.
        let disabled: std::collections::HashSet<String> = self
            .settings
            .get_settings()?
            .skills
            .disabled
            .into_iter()
            .collect();
        // `discovered` is already deduped (highest scope wins), alphabetically
        // sorted, and validation-filtered by `discover_skills`; broken skills
        // are naturally absent and never abort the run (VAL-ASSEMBLE-028).
        let effective: Vec<crate::services::skills::Skill> = discovered
            .into_iter()
            .filter(|s| !disabled.contains(&s.name))
            .collect();

        // Gate strictly on the EFFECTIVE set: only inject when it is non-empty.
        if !effective.is_empty() {
            // (a) System-prompt index. `format_skills_section` returns the
            // `<available_skills>` block (None only for an empty slice, which we
            // already excluded). Compose with the base prompt: base first when
            // present, otherwise the section alone (no leading separator).
            if let Some(section) = crate::services::skills::format_skills_section(&effective) {
                let base = std::mem::take(&mut context.system_prompt);
                context.system_prompt = if base.is_empty() {
                    section
                } else {
                    format!("{base}\n\n{section}")
                };
            }

            // (b) Auto-inject the `skill` tool over a name->body map of the
            // effective set. Bodies live ONLY in the tool's lookup table, never
            // in the system prompt (VAL-TOOL-014).
            let skill_bodies: HashMap<String, String> =
                effective.into_iter().map(|s| (s.name, s.body)).collect();
            tools.push(agent_tools::make_skill_tool(skill_bodies));
        }

        let mut config = AgentLoopConfig::new(model, stream_options);
        // Default to Parallel when unset (matches hand-agent's own default).
        config.tool_execution = tool_execution_mode_from(session.tool_execution_mode.as_deref());

        // run-time steering：在每个 turn 边界 drain 本 run 的 steering 队列，把待注入
        // 的 user 消息交给 loop（loop 会把它们作为 MessageStart/MessageEnd 注入进
        // context，并照常落库）。闭包持有的 `Arc` 与 `RunHandle.steering` 是**同一个**
        // —— `AgentRuntime::steer` 经注册表 push，这里经闭包 drain。`std::mem::take`
        // 一次性取走全部已入队消息并清空队列；空队列时返回空 Vec（loop 无注入）。
        // 默认 `steering_mode == OneAtATime`：loop 每个 turn 调用一次本闭包，故按入队
        // 顺序逐 turn 注入。
        let steering_for_closure = Arc::clone(&steering);
        config.get_steering_messages = Some(Arc::new(move || {
            let queue = Arc::clone(&steering_for_closure);
            Box::pin(async move {
                let mut guard = queue.lock().await;
                std::mem::take(&mut *guard)
            })
        }));

        // 测试专用：用 scripted StreamFn 取代真实网络流，使 loop 走确定性路径。
        #[cfg(test)]
        if let Some(stream_fn) = &self.stream_fn_override {
            config.stream_fn = Some(stream_fn.clone());
        }

        Ok((prompts, context, config, tools))
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
    // 8 args: the per-session `tools` list joins the existing run inputs; they are
    // cohesive run-assembly outputs, not a sign to bundle into a struct here.
    #[allow(clippy::too_many_arguments)]
    fn spawn_loop(
        &self,
        session_id: UUID,
        prompts: Vec<Message>,
        mut context: AgentContext,
        config: AgentLoopConfig,
        tools: Vec<AgentTool>,
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

        let on_event_for_err = Arc::clone(&sink.on_event);
        let on_error = sink.on_error.clone();
        let on_closed = Arc::clone(&sink.on_closed);
        let error_session = session_id.clone();
        let closed_session = session_id;

        // 镜像 `commands/message.rs` 的后台流式模式：spawn 后立即返回，事件经
        // sink 异步抵达。用 `tokio::spawn` 与 `message_user_send_stream` 一致。
        tokio::spawn(async move {
            // 驱动 loop。`cancel` 与 `config.stream_options.base.signal` 是同一个
            // token（见 start_run），也是 `runs` 中登记的那一个 —— abort feature
            // 会从注册表取出它触发取消。
            //
            // 结果分型（见文件头注释）：
            // - `Ok`（含 in-band `AssistantMessageEvent::Error` —— 它在 loop 内被
            //   转成 `stop_reason=Error` 的终结消息并经 `MessageEnd` 正常发出/落库，
            //   loop 仍 `Ok` 返回）：不发 envelope。
            // - `Err(AgentError)`（run-level，如底层 `ProviderNotFound`）：映射成
            //   sanitized 的 `{code,message,hint}`，在 closed **之前**发出 envelope。
            let result = run_agent_loop(
                prompts,
                &mut context,
                &tools,
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

            // run-level Err：在 closed 之前发出 sanitized envelope（无 assistant
            // 内容）。in-band error 是 `Ok`，不会走这里 —— 避免对其重复上报。
            if let Err(err) = &result {
                let app_error = sanitize_agent_error(err);
                let envelope = json!({
                    "sessionId": error_session,
                    "error": app_error,
                });
                match &on_error {
                    Some(emit_error) => emit_error(envelope),
                    // 无专用通道时退回 `on_event`：错误仍抵达 UI，且不新增 closed。
                    None => on_event_for_err(envelope),
                }
            }

            // --- (4) 终结：恰好一次发出 closed，再移除注册表条目 ---
            // 无论 Ok / Err(envelope) / in-band-error，这里都是唯一的 closed emit site。
            on_closed(json!({ "sessionId": closed_session }));
            runs.lock().await.remove(&closed_session);
        });
    }

    /// 中止某个 session 的活跃 run（若有）。
    ///
    /// 从 `runs` 注册表取出该 session 的 `RunHandle.cancel` 并 `.cancel()`。这个
    /// token 与传给 `run_agent_loop` 及 `config.stream_options.base.signal` 的是
    /// **同一个** —— 触发后 hand-agent 的 loop 在下一个 `select!` 边界终止，合成一条
    /// `stopReason=aborted` 的终结 assistant 消息并发出 `MessageStart`/`MessageEnd`
    /// （由既有的 MessageEnd 持久化路径落库），随后 spawned 任务在唯一的 closed
    /// emit site 发出 `agent_stream_closed` 并自行从 `runs` 移除条目。
    ///
    /// 未知 / 已结束的 session 是**干净的 no-op**（不报错、不 panic）—— 前端可能
    /// 竞态地调用 abort（例如 run 刚自然结束）。这里只取消 token，不主动移除条目：
    /// 条目的移除统一由 spawned 任务在 loop 收尾时完成，保持单一所有权。
    pub async fn abort(&self, session_id: &UUID) {
        if let Some(handle) = self.runs.lock().await.get(session_id) {
            handle.cancel.cancel();
        }
    }

    /// 把一条 steering 消息并入某个 session **正在进行**的 run（而非另起并发 run）。
    ///
    /// 从 `runs` 注册表取出该 session 的 `RunHandle.steering` 队列，构造一条 user
    /// `Message` 压入其中；run loop 在下一个 turn 边界经 `get_steering_messages`
    /// 闭包 drain 同一队列并把它注入 context（见 `assemble_run`）。one-run-per-session
    /// 保持成立：steering 喂给既有 run，**不**启动第二个 run。
    ///
    /// - 空 / 纯空白 `text` 是**no-op**：不入队任何东西，run 不受打扰。
    /// - 该 session **无活跃 run** 时也是**干净的 no-op**（不报错）—— 前端可能在 run
    ///   刚自然结束时竞态地调用本方法；返回错误会把这种良性竞态变成噪声。
    pub async fn steer(&self, session_id: &UUID, text: String) {
        if text.trim().is_empty() {
            return;
        }
        if let Some(handle) = self.runs.lock().await.get(session_id) {
            let message = Message::User(UserMessage::new_text(text));
            handle.steering.lock().await.push(message);
        }
    }

    /// 当前活跃 run 数量（测试辅助）。
    #[cfg(test)]
    async fn active_run_count(&self) -> usize {
        self.runs.lock().await.len()
    }

    /// 测试辅助：直接向 `runs` 注册表插入一个 `RunHandle`，返回其取消 token，
    /// 以便在不 spawn 真实 loop 的前提下单测 `abort` 的取消语义。
    #[cfg(test)]
    async fn insert_run_handle(&self, session_id: &UUID) -> CancellationToken {
        let cancel = CancellationToken::new();
        self.runs.lock().await.insert(
            session_id.clone(),
            RunHandle {
                cancel: cancel.clone(),
                steering: Arc::new(Mutex::new(Vec::new())),
            },
        );
        cancel
    }

    /// 测试辅助：返回某个 session 当前已入队的 steering 消息数（不 drain）。
    /// 不存在活跃 run 时返回 `None`。
    #[cfg(test)]
    async fn steering_len(&self, session_id: &UUID) -> Option<usize> {
        match self.runs.lock().await.get(session_id) {
            Some(handle) => Some(handle.steering.lock().await.len()),
            None => None,
        }
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
            project_id: None,
            model_id: Some(model_id.to_string()),
            provider_id: Some(provider_id.to_string()),
            system_prompt: Some("You are a helpful agent.".to_string()),
            thinking_level: None,
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir: None,
            enabled_tools: vec![],
            enabled_skills: vec![],
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

    /// Seed an agent session with an explicit tool configuration —
    /// `enabled_tools`, `working_dir`, and `tool_execution_mode` — so the
    /// backend tool-gating tests can drive the per-session filtered tool list.
    async fn seed_session_with_tools(
        db: &Arc<Database>,
        provider_id: &str,
        model_id: &str,
        enabled_tools: Vec<String>,
        working_dir: Option<String>,
        tool_execution_mode: Option<String>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let session = AgentSession {
            id: id.clone(),
            name: "Tool Session".to_string(),
            project_id: None,
            model_id: Some(model_id.to_string()),
            provider_id: Some(provider_id.to_string()),
            system_prompt: Some("You are a helpful agent.".to_string()),
            thinking_level: None,
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir,
            enabled_tools,
            enabled_skills: vec![],
            tool_execution_mode,
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

    /// A capturing sink: records every event payload, every closed payload, and
    /// every run-level error envelope (the dedicated `on_error` channel).
    #[derive(Clone, Default)]
    struct CapturingSink {
        events: Arc<StdMutex<Vec<serde_json::Value>>>,
        closed: Arc<StdMutex<Vec<serde_json::Value>>>,
        errors: Arc<StdMutex<Vec<serde_json::Value>>>,
    }

    impl CapturingSink {
        fn into_run_sink(self) -> RunSink {
            let events = Arc::clone(&self.events);
            let closed = Arc::clone(&self.closed);
            let errors = Arc::clone(&self.errors);
            RunSink::new(
                Arc::new(move |v| events.lock().unwrap().push(v)),
                Arc::new(move |v| closed.lock().unwrap().push(v)),
            )
            .with_error(Arc::new(move |v| errors.lock().unwrap().push(v)))
        }

        fn closed_count(&self) -> usize {
            self.closed.lock().unwrap().len()
        }

        fn event_count(&self) -> usize {
            self.events.lock().unwrap().len()
        }

        fn error_count(&self) -> usize {
            self.errors.lock().unwrap().len()
        }

        /// The captured run-level error envelopes (`{ sessionId, error }`).
        fn errors(&self) -> Vec<serde_json::Value> {
            self.errors.lock().unwrap().clone()
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
                vec![],
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
                vec![],
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
                vec![],
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

    /// Build a finished `AssistantMessage` whose only content block is a
    /// `ToolCall` for `tool_name`, with `stop_reason == ToolUse`. The loop will
    /// extract this call and dispatch it against the registered tool list (or
    /// return `Tool '<name>' not found` when it is absent).
    fn tool_call_message(
        model_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> AssistantMessage {
        AssistantMessage {
            role: "assistant".into(),
            content: vec![model::AssistantContentBlock::ToolCall(
                model::ToolCall::new(tool_call_id, tool_name, args),
            )],
            api: model::Api::OpenAICompletions,
            provider: model::types::Provider::OpenAI,
            model: model_id.to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::ToolUse,
            error_message: None,
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        }
    }

    /// A two-turn scripted stream: the FIRST invocation emits an assistant
    /// message carrying a single `ToolCall` (so the loop dispatches the tool);
    /// the SECOND (and any later) invocation emits a plain `Stop` so the loop
    /// terminates after the tool result is appended. Stateful via an atomic
    /// turn counter so the run reaches a clean terminal MessageEnd.
    fn tool_then_done_stream_fn(
        tool_call_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> hand_agent::StreamFn {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let turn = Arc::new(AtomicUsize::new(0));
        let tool_call_id = tool_call_id.to_string();
        let tool_name = tool_name.to_string();
        Arc::new(move |model, _ctx, _opts, _cancel| {
            let n = turn.fetch_add(1, Ordering::SeqCst);
            let events = if n == 0 {
                let msg = tool_call_message(&model.id, &tool_call_id, &tool_name, args.clone());
                vec![
                    AssistantMessageEvent::Start {
                        partial: msg.clone(),
                    },
                    AssistantMessageEvent::Done {
                        reason: StopReason::ToolUse,
                        message: msg,
                    },
                ]
            } else {
                vec![
                    AssistantMessageEvent::Start {
                        partial: done_message(&model.id, ""),
                    },
                    AssistantMessageEvent::Done {
                        reason: StopReason::Stop,
                        message: done_message(&model.id, "done after tool"),
                    },
                ]
            };
            Box::pin(futures::stream::iter(events))
        })
    }

    /// Read the persisted `toolResult` rows for a session (role `toolResult`),
    /// ordered by `seq`, returning each row's `isError` flag and full payload
    /// text so gating tests can assert on the stored tool outcome.
    async fn query_tool_results(db: &Arc<Database>, session_id: &str) -> Vec<(bool, String)> {
        let rows = sqlx::query(
            r#"
            SELECT json_extract(payload, '$.isError') AS is_error,
                   payload
            FROM agent_session_messages
            WHERE session_id = $1 AND role = 'toolResult'
            ORDER BY seq ASC
        "#,
        )
        .bind(session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        rows.into_iter()
            .map(|row| {
                let is_error: Option<i64> = row.try_get("is_error").ok().flatten();
                let payload: String = row.try_get("payload").unwrap();
                (is_error.unwrap_or(0) != 0, payload)
            })
            .collect()
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
                vec![],
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
                vec![],
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
                vec![],
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
                vec![],
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

    // -------------------------------------------------------------------------
    // Error classification: envelope (run-level Err) vs in-band error.
    // -------------------------------------------------------------------------

    /// A seeded fake API key. Tests assert it NEVER leaks into the emitted
    /// error envelope nor any persisted `errorMessage` (D24/D14).
    const FAKE_KEY: &str = "sk-LEAKME-deadbeefcafebabe-secret-key";

    /// Build a terminal assistant `Message` carrying a provider business error:
    /// `stop_reason == Error` plus an `errorMessage` that (adversarially) embeds
    /// the seeded fake key, so the no-leak assertion is meaningful.
    fn errored_message(model_id: &str, error_text: &str) -> AssistantMessage {
        AssistantMessage {
            role: "assistant".into(),
            content: vec![],
            api: model::Api::OpenAICompletions,
            provider: model::types::Provider::OpenAI,
            model: model_id.to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Error,
            error_message: Some(error_text.to_string()),
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        }
    }

    /// A model whose `api` (`Faux`) is NOT registered in `shared_client()`
    /// (builtins only), so the real `client.stream_simple` returns
    /// `ProviderNotFound` -> `run_agent_loop` returns a real
    /// `Err(AgentError::Client(..))`. Drives the envelope path through the
    /// genuine code path without enabling the `faux` Cargo feature.
    fn unregistered_api_model(model_id: &str) -> model::Model {
        model::Model {
            id: model_id.to_string(),
            name: model_id.to_string(),
            api: model::Api::Faux,
            provider: model::types::Provider::OpenAI,
            base_url: String::new(),
            reasoning: false,
            input: vec![model::InputType::Text],
            cost: model::Cost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
            },
            context_window: 0,
            max_tokens: 0,
            headers: None,
            compat: None,
            thinking_level_map: None,
        }
    }

    /// VAL-RUN-012: a provider-not-found run (real `Err(AgentError)` from
    /// `run_agent_loop`) emits a sanitized `agent_stream_error{code,message,hint}`
    /// envelope BEFORE the terminal closed, with NO assistant content persisted,
    /// and then EXACTLY ONE closed. The envelope carries a stable AppError code
    /// and never echoes a raw provider/transport error verbatim.
    #[tokio::test]
    async fn provider_not_found_emits_sanitized_envelope_then_one_closed() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        // No stream_fn: drive the real `client.stream_simple`, which returns
        // ProviderNotFound for this unregistered-API model.
        runtime.model_override = Some(unregistered_api_model("gpt-4o"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hello".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed (the failure is in-loop)");

        wait_for_closed(&sink).await;

        // Exactly one error envelope and exactly one closed.
        assert_eq!(sink.error_count(), 1, "exactly one agent_stream_error");
        assert_eq!(sink.closed_count(), 1, "exactly one agent_stream_closed");

        let envelope = sink.errors().remove(0);
        assert_eq!(
            envelope.get("sessionId").and_then(|v| v.as_str()),
            Some(session_id.as_str()),
            "envelope carries the session id"
        );
        let error = envelope.get("error").expect("envelope has an error object");
        let code = error.get("code").and_then(|v| v.as_str()).unwrap_or("");
        // A stable AppError code from the project vocabulary.
        assert!(
            matches!(
                code,
                "AUTH_ERROR" | "NETWORK_ERROR" | "RATE_LIMIT" | "INTERNAL_ERROR"
            ),
            "envelope code must be from the AppError vocabulary, got {code:?}"
        );
        assert!(
            error.get("message").and_then(|v| v.as_str()).is_some(),
            "envelope has a message"
        );
        assert!(
            error.get("hint").and_then(|v| v.as_str()).is_some(),
            "envelope has a hint"
        );

        // No assistant content was persisted on the run-level Err path: the
        // failure happens before any assistant MessageEnd. Only the user prompt
        // (emitted before streaming) lands.
        let rows = query_messages(&db, &session_id).await;
        assert!(
            rows.iter().all(|r| r.role != "assistant"),
            "no assistant content persisted on the envelope path"
        );

        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-RUN-013: an invalid-key / in-band provider error (the stream yields
    /// `AssistantMessageEvent::Error`) produces a terminal assistant message with
    /// `stopReason=error` that flows through the NORMAL event/persist path — NO
    /// `agent_stream_error` envelope is emitted — and ends with exactly one
    /// closed. The seeded fake key never leaks into the envelope (there is none)
    /// nor (we assert) into the run-level error mapping.
    #[tokio::test]
    async fn in_band_error_persists_stop_reason_error_no_envelope() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // A scripted stream that ends in an Error event whose errorMessage
        // adversarially embeds the fake key (as a leaky provider might).
        let leaky_text = format!("401 Unauthorized: invalid api key {FAKE_KEY}");
        let stream_fn: hand_agent::StreamFn = Arc::new(move |model, _ctx, _opts, _cancel| {
            let events = vec![AssistantMessageEvent::Error {
                reason: StopReason::Error,
                error: errored_message(&model.id, &leaky_text),
            }];
            Box::pin(futures::stream::iter(events))
        });

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(stream_fn);

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hello".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        // In-band: NO envelope, exactly one closed.
        assert_eq!(
            sink.error_count(),
            0,
            "in-band error must NOT produce an agent_stream_error envelope"
        );
        assert_eq!(sink.closed_count(), 1, "exactly one agent_stream_closed");

        // The terminal assistant message persisted with stopReason=error.
        let rows = sqlx::query(
            r#"
            SELECT role,
                   json_extract(payload, '$.stopReason') AS stop_reason,
                   json_extract(payload, '$.errorMessage') AS error_message
            FROM agent_session_messages
            WHERE session_id = $1 AND role = 'assistant'
            ORDER BY seq ASC
        "#,
        )
        .bind(&session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "one terminal assistant row persisted");
        let stop_reason: Option<String> = rows[0].try_get("stop_reason").ok().flatten();
        assert_eq!(
            stop_reason.as_deref(),
            Some("error"),
            "terminal assistant persisted with stopReason=error"
        );

        // The in-band errorMessage is preserved verbatim (it is in-band content,
        // NOT a sanitized envelope) — this documents that the envelope is the
        // surface our sanitizer guards, while in-band content is the provider's
        // own message. The sanitizer is exercised on the envelope path test;
        // here we confirm no envelope was emitted that could leak via mapping.
        assert!(
            sink.errors().is_empty(),
            "no run-level error envelope was emitted for the in-band path"
        );
    }

    /// VAL-RUN-013 (sanitizer no-leak, isolated): the run-level error mapping
    /// never echoes a raw provider error that could carry the API key. We drive
    /// the real `ProviderNotFound` envelope and assert the seeded fake key never
    /// appears in the emitted `{code,message,hint}`.
    #[tokio::test]
    async fn envelope_mapping_does_not_leak_api_key() {
        let (db, _guard) = test_db().await;
        // Seed a provider whose api_key is the fake key itself.
        let id = uuid::Uuid::new_v4().to_string();
        let provider = Provider {
            id: id.clone(),
            name: "Leaky".to_string(),
            provider_type: "openai".to_string(),
            base_url: String::new(),
            api_key: FAKE_KEY.to_string(),
            enabled: true,
            created_at: now_ms(),
            updated_at: now_ms(),
        };
        ProviderRepository::new(Arc::clone(&db))
            .create_provider(&provider)
            .await
            .unwrap();
        let session_id = seed_session(&db, &id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.model_override = Some(unregistered_api_model("gpt-4o"));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hi".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        assert_eq!(sink.error_count(), 1, "one envelope emitted");
        let envelope = sink.errors().remove(0);
        let serialized = serde_json::to_string(&envelope).unwrap();
        assert!(
            !serialized.contains(FAKE_KEY),
            "the API key must never appear in the error envelope: {serialized}"
        );
    }

    /// VAL-RUN-014: a network-drop-mid-stream run (text streamed, THEN an Error
    /// event) keeps the already-streamed partial content (persisted at its
    /// terminal MessageEnd as a stopReason=error assistant message) and ends
    /// with exactly one closed. No envelope (in-band path).
    #[tokio::test]
    async fn network_drop_mid_stream_preserves_partial_then_one_closed() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // Stream some text, then drop with an Error. The Error event's message
        // carries the partial text content plus stop_reason=Error.
        let stream_fn: hand_agent::StreamFn = Arc::new(move |model, _ctx, _opts, _cancel| {
            let partial = done_message(&model.id, "partial answer before drop");
            let mut errored = errored_message(&model.id, "connection reset by peer");
            errored.content = partial.content.clone();
            let events = vec![
                AssistantMessageEvent::Start {
                    partial: done_message(&model.id, ""),
                },
                AssistantMessageEvent::TextDelta {
                    content_index: 0,
                    delta: "partial answer before drop".into(),
                    partial,
                },
                AssistantMessageEvent::Error {
                    reason: StopReason::Error,
                    error: errored,
                },
            ];
            Box::pin(futures::stream::iter(events))
        });

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(stream_fn);

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "ask".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_closed(&sink).await;

        assert_eq!(sink.error_count(), 0, "in-band drop emits no envelope");
        assert_eq!(sink.closed_count(), 1, "exactly one agent_stream_closed");

        // The partial content survived: a terminal assistant row holding the
        // streamed text persisted (at its MessageEnd), with stopReason=error.
        let rows = sqlx::query(
            r#"
            SELECT role,
                   json_extract(payload, '$.stopReason') AS stop_reason,
                   payload
            FROM agent_session_messages
            WHERE session_id = $1 AND role = 'assistant'
            ORDER BY seq ASC
        "#,
        )
        .bind(&session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "the partial assistant message persisted");
        let stop_reason: Option<String> = rows[0].try_get("stop_reason").ok().flatten();
        assert_eq!(stop_reason.as_deref(), Some("error"));
        let payload: String = rows[0].try_get("payload").unwrap();
        assert!(
            payload.contains("partial answer before drop"),
            "the streamed partial content is preserved in the persisted row: {payload}"
        );
    }

    // -------------------------------------------------------------------------
    // Abort: mid-run cancel, abort-before-first-token, unknown-session no-op,
    // and the delete-mid-run abort semantics at the `AgentRuntime::abort` level.
    // -------------------------------------------------------------------------

    /// A scripted stream that emits the given events, then PENDS FOREVER on the
    /// next poll — so the loop blocks at `stream.next()` inside its cancellation
    /// `select!`, leaving the run "in progress" until `abort()` fires. The token
    /// the loop selects on is the same one registered in `runs`.
    fn hanging_stream_fn(events: Vec<AssistantMessageEvent>) -> hand_agent::StreamFn {
        use futures::StreamExt;
        let events = Arc::new(StdMutex::new(Some(events)));
        Arc::new(move |_model, _ctx, _opts, _cancel| {
            let scripted = events.lock().unwrap().take().unwrap_or_default();
            Box::pin(futures::stream::iter(scripted).chain(futures::stream::pending()))
        })
    }

    /// Drive `abort()` from the test once the run is observed in flight, then
    /// wait for the terminal closed. Polls the registry so the cancel fires only
    /// after the run is registered (avoids racing `start_run`'s placeholder).
    async fn abort_when_active(runtime: &AgentRuntime, session_id: &str, sink: &CapturingSink) {
        for _ in 0..200 {
            if runtime.active_run_count().await >= 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        runtime.abort(&session_id.to_string()).await;
        wait_for_closed(sink).await;
    }

    /// VAL-RUN-007: aborting mid-stream ends the run with a terminal assistant
    /// row carrying `stopReason=aborted`, while content that reached a
    /// `MessageEnd` before the abort remains persisted, and EXACTLY ONE closed
    /// fires. The pre-abort content is modeled as a prior completed transcript
    /// row (a finished earlier turn); the aborted turn's stream is gated open so
    /// the cancel lands while the loop is awaiting the next chunk.
    #[tokio::test]
    async fn abort_mid_run_persists_aborted_terminal_and_retains_prior_content() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        // Pre-abort content: a completed earlier turn (user + assistant), each
        // already persisted at its own MessageEnd (seeded directly here). The
        // payloads are valid hand-agent `Message`s so the transcript replay in
        // `assemble_run` succeeds.
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

        // The aborted turn: emit Start + a partial TextDelta, then hang. No Done
        // arrives, so the loop blocks at `stream.next()` until abort cancels it.
        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(hanging_stream_fn(vec![
            AssistantMessageEvent::Start {
                partial: done_message("gpt-4o", ""),
            },
            AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "streaming before abort".into(),
                partial: done_message("gpt-4o", "streaming before abort"),
            },
        ]));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "abort me".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        abort_when_active(&runtime, &session_id, &sink).await;

        // Exactly one closed on the abort path (no second emit site), no run-level
        // error envelope (abort is a clean Ok at the loop level).
        assert_eq!(
            sink.closed_count(),
            1,
            "exactly one agent_stream_closed on abort"
        );
        assert_eq!(sink.error_count(), 0, "abort emits no error envelope");

        // The prior completed content survived, AND a terminal aborted row exists.
        let rows = sqlx::query(
            r#"
            SELECT role,
                   json_extract(payload, '$.stopReason') AS stop_reason,
                   payload
            FROM agent_session_messages
            WHERE session_id = $1
            ORDER BY seq ASC
        "#,
        )
        .bind(&session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();

        let payloads: Vec<String> = rows.iter().map(|r| r.try_get("payload").unwrap()).collect();
        assert!(
            payloads.iter().any(|p| p.contains("earlier answer")),
            "pre-abort completed content remains persisted: {payloads:?}"
        );

        let stop_reasons: Vec<Option<String>> = rows
            .iter()
            .map(|r| r.try_get::<Option<String>, _>("stop_reason").ok().flatten())
            .collect();
        assert!(
            stop_reasons.iter().any(|s| s.as_deref() == Some("aborted")),
            "a terminal assistant row with stopReason=aborted is persisted: {stop_reasons:?}"
        );

        // The run drained from the registry after the abort closed it.
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// Assertion (2): aborting BEFORE the first token ends cleanly — no stuck
    /// running state, EXACTLY ONE closed, the registry empties. The stream hangs
    /// from the very first poll (no events), so the abort lands before any
    /// MessageStart; the synthesized aborted message is the only assistant row.
    #[tokio::test]
    async fn abort_before_first_token_ends_clean_one_closed() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        // No scripted events: hang immediately so abort fires before the first token.
        runtime.stream_fn_override = Some(hanging_stream_fn(vec![]));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "abort early".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        abort_when_active(&runtime, &session_id, &sink).await;

        assert_eq!(sink.closed_count(), 1, "exactly one agent_stream_closed");
        assert_eq!(sink.error_count(), 0, "abort emits no error envelope");

        // No stuck running state: the registry is empty after the clean abort.
        assert_eq!(
            runtime.active_run_count().await,
            0,
            "no stuck running state after abort-before-first-token"
        );

        // The synthesized terminal carries stopReason=aborted.
        let rows = sqlx::query(
            r#"
            SELECT json_extract(payload, '$.stopReason') AS stop_reason
            FROM agent_session_messages
            WHERE session_id = $1 AND role = 'assistant'
            ORDER BY seq ASC
        "#,
        )
        .bind(&session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(rows.len(), 1, "one synthesized aborted assistant row");
        let stop_reason: Option<String> = rows[0].try_get("stop_reason").ok().flatten();
        assert_eq!(stop_reason.as_deref(), Some("aborted"));
    }

    /// `abort()` on an unknown / already-finished session is a clean no-op: it
    /// must not error or panic (the frontend may call it racily). Asserted both
    /// before any run exists and after a run has fully drained.
    #[tokio::test]
    async fn abort_unknown_session_is_clean_no_op() {
        let (db, _guard) = test_db().await;

        let runtime = AgentRuntime::new(Arc::clone(&db));

        // No run was ever registered for this id — must be a silent no-op.
        runtime.abort(&"never-existed".to_string()).await;
        assert_eq!(runtime.active_run_count().await, 0);

        // After a finished run, aborting the same id again is also a no-op.
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;
        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(single_turn_stream_fn("done"));
        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "hi".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .unwrap();
        wait_for_closed(&sink).await;
        for _ in 0..200 {
            if runtime.active_run_count().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        // The run already drained; aborting again must not error or panic.
        runtime.abort(&session_id).await;
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-SESSION-010 (at the `AgentRuntime::abort` level): the
    /// delete-mid-run path calls `runtime.abort(id)` before deleting. Tested
    /// directly on the registry: insert a `RunHandle`, call `abort`, and assert
    /// the registered cancel token is cancelled (so the loop would terminate and
    /// stop emitting `agent_stream_event` for the deleted session id). The entry
    /// is left for the spawned task to remove (single ownership of removal); here
    /// no task drives it, so it stays — what matters is the token is cancelled.
    #[tokio::test]
    async fn abort_cancels_registered_token_for_delete_mid_run() {
        let (db, _guard) = test_db().await;
        let runtime = AgentRuntime::new(Arc::clone(&db));

        let session_id = "mid-run-session".to_string();
        let token = runtime.insert_run_handle(&session_id).await;
        assert!(!token.is_cancelled(), "token starts uncancelled");
        assert_eq!(runtime.active_run_count().await, 1, "run handle registered");

        // The delete-mid-run path's first action: abort the active run.
        runtime.abort(&session_id).await;

        assert!(
            token.is_cancelled(),
            "abort cancelled the registered token — the loop would terminate, \
             halting further agent_stream_event for the deleted session"
        );
    }

    // -------------------------------------------------------------------------
    // Backend tool gating (D8/D9) + secret/path hygiene (D14).
    //
    // The gate is structural: `build_tools` only registers the session's
    // enabled, supported, read-only tools, so the loop's own
    // `Tool '<name>' not found` (is_error=true) is the enforcement for a
    // disabled tool, the FS tools without a working_dir, and the mutating tool
    // names that are never registered. These tests script an assistant tool_call
    // and assert the resulting persisted `toolResult` row.
    // -------------------------------------------------------------------------

    /// Map a session's `tool_execution_mode` string to hand-agent's enum:
    /// `"sequential"` (case-insensitive, trimmed) → Sequential; everything else
    /// — unset, unknown, blank — defaults to Parallel (hand-agent's default).
    #[test]
    fn tool_execution_mode_defaults_to_parallel_unless_sequential() {
        assert_eq!(tool_execution_mode_from(None), ToolExecutionMode::Parallel);
        assert_eq!(
            tool_execution_mode_from(Some("sequential")),
            ToolExecutionMode::Sequential
        );
        assert_eq!(
            tool_execution_mode_from(Some("  Sequential  ")),
            ToolExecutionMode::Sequential
        );
        assert_eq!(
            tool_execution_mode_from(Some("parallel")),
            ToolExecutionMode::Parallel
        );
        // Unknown / blank strings fall back to the default rather than erroring.
        assert_eq!(
            tool_execution_mode_from(Some("nonsense")),
            ToolExecutionMode::Parallel
        );
        assert_eq!(
            tool_execution_mode_from(Some("")),
            ToolExecutionMode::Parallel
        );
    }

    /// VAL-TOOLS-007: a session with a built-in tool toggled OFF. When the model
    /// emits that tool call, the loop yields a `toolResult` with `is_error`
    /// containing "not found" — the tool does not execute (it was never
    /// registered, because it is not in `enabled_tools`).
    #[tokio::test]
    async fn disabled_tool_yields_tool_not_found_and_does_not_execute() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        // `web_fetch` is enabled, but the model calls `read_file`, which is OFF.
        let session_id = seed_session_with_tools(
            &db,
            &provider_id,
            "gpt-4o",
            vec!["web_fetch".to_string()],
            Some("/tmp".to_string()),
            None,
        )
        .await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(tool_then_done_stream_fn(
            "tc-disabled",
            "read_file",
            json!({"path": "inside.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "use the disabled tool".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");
        wait_for_closed(&sink).await;

        let results = query_tool_results(&db, &session_id).await;
        assert_eq!(results.len(), 1, "exactly one tool result persisted");
        let (is_error, payload) = &results[0];
        assert!(*is_error, "the disabled-tool result is an error result");
        assert!(
            payload.contains("not found"),
            "toolResult must say the tool was not found, got: {payload}"
        );
    }

    /// VAL-TOOLS-008 (registration half): with an empty/None `working_dir` the FS
    /// tools (`read_file`/`list_directory`) are NOT registered, while `web_fetch`
    /// IS — `web_fetch` needs no sandbox root. Asserted directly on the per-
    /// session `build_tools` output (the same call the assemble path makes).
    #[test]
    fn empty_working_dir_omits_fs_tools_keeps_web_fetch() {
        let enabled = vec![
            "read_file".to_string(),
            "list_directory".to_string(),
            "web_fetch".to_string(),
        ];

        // None working_dir.
        let names: Vec<String> = agent_tools::build_tools(&enabled, None)
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            names,
            vec!["web_fetch".to_string()],
            "None working_dir: only web_fetch is registered"
        );

        // Empty-string working_dir behaves the same as None.
        let empty = Path::new("");
        let names_empty: Vec<String> = agent_tools::build_tools(&enabled, Some(empty))
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            names_empty,
            vec!["web_fetch".to_string()],
            "empty working_dir: only web_fetch is registered"
        );
    }

    /// VAL-TOOLS-008 (runtime half): with no working_dir, a model `read_file`
    /// call resolves to tool-not-found through the real run loop (the FS tool was
    /// never registered), confirming the structural gate, not just `build_tools`.
    #[tokio::test]
    async fn no_working_dir_read_file_call_is_tool_not_found() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session_with_tools(
            &db,
            &provider_id,
            "gpt-4o",
            vec!["read_file".to_string(), "web_fetch".to_string()],
            None, // no working_dir → read_file omitted
            None,
        )
        .await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(tool_then_done_stream_fn(
            "tc-nofs",
            "read_file",
            json!({"path": "x.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "read a file".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");
        wait_for_closed(&sink).await;

        let results = query_tool_results(&db, &session_id).await;
        assert_eq!(results.len(), 1);
        let (is_error, payload) = &results[0];
        assert!(
            *is_error,
            "read_file without a sandbox root is not runnable"
        );
        assert!(
            payload.contains("not found"),
            "read_file without working_dir is tool-not-found, got: {payload}"
        );
    }

    /// VAL-TOOLS-011: the mutating tool names (`write_file`/`run_command`) are
    /// NEVER registered — `build_tools` ignores unknown names — so a call to one
    /// resolves to tool-not-found and (because there is no executor) nothing is
    /// written to disk. We point `working_dir` at a temp dir and assert no file
    /// the call "asked for" appears there.
    #[tokio::test]
    async fn mutating_tools_are_never_registered_and_write_nothing() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let work_path = work.path().to_string_lossy().into_owned();
        let target = work.path().join("created-by-write.txt");

        // The session even tries to enable the mutating names; build_tools must
        // still refuse to produce them.
        let session_id = seed_session_with_tools(
            &db,
            &provider_id,
            "gpt-4o",
            vec![
                "write_file".to_string(),
                "run_command".to_string(),
                "read_file".to_string(),
            ],
            Some(work_path.clone()),
            None,
        )
        .await;

        // build_tools must omit both mutating names regardless of working_dir.
        let names: Vec<String> = agent_tools::build_tools(
            &[
                "write_file".to_string(),
                "run_command".to_string(),
                "read_file".to_string(),
            ],
            Some(work.path()),
        )
        .into_iter()
        .map(|t| t.name)
        .collect();
        assert!(
            !names
                .iter()
                .any(|n| n == "write_file" || n == "run_command"),
            "mutating tools must never be registered, got: {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "read_file"),
            "the read-only tool is still registered: {names:?}"
        );

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(tool_then_done_stream_fn(
            "tc-write",
            "write_file",
            json!({"path": "created-by-write.txt", "content": "pwned"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "write a file".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");
        wait_for_closed(&sink).await;

        let results = query_tool_results(&db, &session_id).await;
        assert_eq!(results.len(), 1);
        let (is_error, payload) = &results[0];
        assert!(*is_error, "write_file call is an error (not registered)");
        assert!(
            payload.contains("not found"),
            "write_file is tool-not-found, got: {payload}"
        );
        // Nothing was written to disk: the file the call named does not exist.
        assert!(
            !target.exists(),
            "no mutating tool ran, so the target file must not exist: {target:?}"
        );
    }

    /// VAL-TOOLS-012: during a tool-bearing run seeded with a provider key, the
    /// key never appears in (a) the emitted `agent_stream_event` payloads, nor
    /// (b) the persisted `agent_session_messages.payload` (including the
    /// `toolResult` rows). The run scripts a tool_call so the tool-dispatch path
    /// is exercised; the tool is not registered (disabled) so we also confirm the
    /// gate, but the focus here is the no-key-leak invariant.
    #[tokio::test]
    async fn provider_key_never_leaks_in_events_or_persisted_payload() {
        let (db, _guard) = test_db().await;

        // Seed a provider whose api_key is the adversarial fake key.
        let provider_id = uuid::Uuid::new_v4().to_string();
        let provider = Provider {
            id: provider_id.clone(),
            name: "Leaky Tools".to_string(),
            provider_type: "openai".to_string(),
            base_url: String::new(),
            api_key: FAKE_KEY.to_string(),
            enabled: true,
            created_at: now_ms(),
            updated_at: now_ms(),
        };
        ProviderRepository::new(Arc::clone(&db))
            .create_provider(&provider)
            .await
            .unwrap();

        let session_id = seed_session_with_tools(
            &db,
            &provider_id,
            "gpt-4o",
            vec!["web_fetch".to_string()],
            Some("/tmp".to_string()),
            None,
        )
        .await;

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        // The model calls a disabled tool; the dispatch path runs and yields a
        // tool-not-found result. No real network I/O, but the key is in the
        // assembled stream options for the whole run.
        runtime.stream_fn_override = Some(tool_then_done_stream_fn(
            "tc-leak",
            "read_file",
            json!({"path": "inside.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "fetch and read".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");
        wait_for_closed(&sink).await;

        // (a) No emitted event payload carries the key.
        let events = sink.events.lock().unwrap().clone();
        assert!(!events.is_empty(), "the run emitted events");
        for ev in &events {
            let serialized = serde_json::to_string(ev).unwrap();
            assert!(
                !serialized.contains(FAKE_KEY),
                "the API key leaked into an emitted event: {serialized}"
            );
        }

        // (b) No persisted payload — including the toolResult row — carries it.
        let rows = sqlx::query("SELECT payload FROM agent_session_messages WHERE session_id = $1")
            .bind(&session_id)
            .fetch_all(db.pool())
            .await
            .unwrap();
        assert!(!rows.is_empty(), "the run persisted transcript rows");
        for row in &rows {
            let payload: String = row.try_get("payload").unwrap();
            assert!(
                !payload.contains(FAKE_KEY),
                "the API key leaked into a persisted payload: {payload}"
            );
        }

        // Sanity: the tool-bearing path actually produced a (gated) tool result.
        let results = query_tool_results(&db, &session_id).await;
        assert_eq!(results.len(), 1, "the tool dispatch path was exercised");
        assert!(results[0].0, "disabled tool → error result (gate held)");
    }

    // -------------------------------------------------------------------------
    // Run-time steering (VAL-RUN-017): a steer submitted during an active run is
    // delivered INTO that run via `get_steering_messages` at the next turn
    // boundary — incorporated as a user message — WITHOUT starting a second run;
    // an empty/whitespace steer is a no-op.
    //
    // The crux is that `RunHandle.steering` and the `get_steering_messages`
    // closure share the SAME `Arc`: the command pushes via the registry handle,
    // the loop drains via the closure. The tests drive this through the real run
    // loop with a scripted multi-turn stream (a turn boundary must exist for the
    // loop to poll steering after the first turn).
    // -------------------------------------------------------------------------

    /// A two-turn scripted stream whose FIRST turn is GATED on a release signal,
    /// emitting an assistant `ToolCall` only after the signal fires; the SECOND
    /// (and any later) turn emits a plain `Stop`. Gating the first turn keeps the
    /// run reliably "in flight" so a test can `steer` into it; once released, the
    /// loop finishes the tool turn, then polls steering at the turn boundary and
    /// injects the queued message before the second turn.
    fn gated_tool_then_done_stream_fn(
        release_rx: tokio::sync::oneshot::Receiver<()>,
        tool_call_id: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> hand_agent::StreamFn {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let turn = Arc::new(AtomicUsize::new(0));
        let release_rx = Arc::new(tokio::sync::Mutex::new(Some(release_rx)));
        let tool_call_id = tool_call_id.to_string();
        let tool_name = tool_name.to_string();
        Arc::new(move |model, _ctx, _opts, _cancel| {
            let n = turn.fetch_add(1, Ordering::SeqCst);
            let release_rx = Arc::clone(&release_rx);
            let model_id = model.id.clone();
            let tool_call_id = tool_call_id.clone();
            let tool_name = tool_name.clone();
            let args = args.clone();
            if n == 0 {
                // First turn: block until released, then emit the tool call.
                Box::pin(futures::stream::once(async move {
                    if let Some(rx) = release_rx.lock().await.take() {
                        let _ = rx.await;
                    }
                    let msg = tool_call_message(&model_id, &tool_call_id, &tool_name, args);
                    AssistantMessageEvent::Done {
                        reason: StopReason::ToolUse,
                        message: msg,
                    }
                }))
            } else {
                // Later turns: terminate with a plain Stop.
                let events = vec![AssistantMessageEvent::Done {
                    reason: StopReason::Stop,
                    message: done_message(&model_id, "done after steering"),
                }];
                Box::pin(futures::stream::iter(events))
            }
        })
    }

    /// Poll until the session has an active run registered (or panic on timeout),
    /// so a steer lands while the run is genuinely in flight.
    async fn wait_for_active(runtime: &AgentRuntime, _session_id: &str) {
        for _ in 0..200 {
            if runtime.active_run_count().await >= 1 {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("run did not become active within timeout");
    }

    /// VAL-RUN-017 (delivery): a steer submitted during an active run is drained
    /// by `get_steering_messages` at the turn boundary and incorporated as a user
    /// message into THAT run — proven by a second `user` transcript row carrying
    /// the steered text, persisted after the first turn — and NO second run is
    /// started (the registry holds exactly one entry while the run is in flight).
    #[tokio::test]
    async fn steer_during_active_run_is_incorporated_as_user_message() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        // First turn (gated) emits a disabled tool call → tool-not-found result,
        // the loop continues to a second turn, polling steering in between.
        runtime.stream_fn_override = Some(gated_tool_then_done_stream_fn(
            release_rx,
            "tc-steer",
            "read_file",
            json!({"path": "x.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "initial prompt".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        // The run is in flight (first turn gated). Steer into it.
        wait_for_active(&runtime, &session_id).await;
        assert_eq!(
            runtime.active_run_count().await,
            1,
            "exactly one run is active before steering"
        );
        runtime
            .steer(&session_id, "steered follow-up".to_string())
            .await;
        // The steered message is queued on THIS run (drained at the turn boundary).
        assert_eq!(
            runtime.steering_len(&session_id).await,
            Some(1),
            "the steered message is queued on the active run"
        );
        // Steering started no second run — still exactly one entry.
        assert_eq!(
            runtime.active_run_count().await,
            1,
            "steering does not start a second concurrent run"
        );

        // Release the gated first turn; the run finishes (turn 1 → steering drain
        // → turn 2 → Stop).
        let _ = release_tx.send(());
        wait_for_closed(&sink).await;

        // The steered text was incorporated as a user message into the run: a
        // user transcript row carrying it was persisted (the loop emitted
        // MessageStart/MessageEnd for the drained steering message).
        let user_payloads = sqlx::query(
            r#"
            SELECT payload
            FROM agent_session_messages
            WHERE session_id = $1 AND role = 'user'
            ORDER BY seq ASC
        "#,
        )
        .bind(&session_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        let payloads: Vec<String> = user_payloads
            .iter()
            .map(|r| r.try_get::<String, _>("payload").unwrap())
            .collect();
        assert!(
            payloads.iter().any(|p| p.contains("steered follow-up")),
            "the steered text was incorporated as a user message into the run: {payloads:?}"
        );
        // Both the initial prompt and the steered follow-up are present as user
        // messages — the steer joined the SAME run rather than spawning another.
        assert!(
            payloads.iter().any(|p| p.contains("initial prompt")),
            "the initial prompt user message is present: {payloads:?}"
        );

        // The single run drained from the registry after closing.
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-RUN-017 (empty steer no-op): an empty / whitespace-only steer queues
    /// NOTHING — the active run's steering queue stays empty and the run is
    /// undisturbed (no extra user message is injected).
    #[tokio::test]
    async fn empty_or_whitespace_steer_is_a_no_op() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(gated_tool_then_done_stream_fn(
            release_rx,
            "tc-empty",
            "read_file",
            json!({"path": "x.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "initial prompt".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_active(&runtime, &session_id).await;

        // Empty and whitespace-only steers must queue nothing.
        runtime.steer(&session_id, String::new()).await;
        runtime.steer(&session_id, "   \t\n ".to_string()).await;
        assert_eq!(
            runtime.steering_len(&session_id).await,
            Some(0),
            "empty/whitespace steer queues nothing"
        );

        let _ = release_tx.send(());
        wait_for_closed(&sink).await;

        // The run is undisturbed: only the initial prompt user message exists,
        // no steering-injected user row.
        let user_rows = sqlx::query(
            "SELECT COUNT(*) AS n FROM agent_session_messages WHERE session_id = $1 AND role = 'user'",
        )
        .bind(&session_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let n: i64 = user_rows.try_get("n").unwrap();
        assert_eq!(n, 1, "only the initial prompt user message; no steered row");

        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// VAL-RUN-017 (single run, no second entry): steering an active run never
    /// adds a second `runs` entry — the registry holds exactly one entry for the
    /// session across multiple steers while the run is in flight.
    #[tokio::test]
    async fn steering_does_not_create_a_second_run_entry() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;
        let session_id = seed_session(&db, &provider_id, "gpt-4o").await;

        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();

        let mut runtime = AgentRuntime::new(Arc::clone(&db));
        runtime.stream_fn_override = Some(gated_tool_then_done_stream_fn(
            release_rx,
            "tc-single",
            "read_file",
            json!({"path": "x.txt"}),
        ));

        let sink = CapturingSink::default();
        runtime
            .start_run(
                session_id.clone(),
                "initial prompt".to_string(),
                vec![],
                sink.clone().into_run_sink(),
            )
            .await
            .expect("start_run should succeed");

        wait_for_active(&runtime, &session_id).await;

        // Multiple steers, all feeding the one active run.
        runtime.steer(&session_id, "first steer".to_string()).await;
        runtime.steer(&session_id, "second steer".to_string()).await;
        assert_eq!(
            runtime.active_run_count().await,
            1,
            "the runs map still holds exactly one entry for the session"
        );
        assert_eq!(
            runtime.steering_len(&session_id).await,
            Some(2),
            "both steers queued on the single active run"
        );

        let _ = release_tx.send(());
        wait_for_closed(&sink).await;
        assert_eq!(runtime.active_run_count().await, 0);
    }

    /// Steering a session with NO active run is a clean no-op (does not error or
    /// panic) — the frontend may call it racily as a run naturally ends.
    #[tokio::test]
    async fn steer_unknown_session_is_clean_no_op() {
        let (db, _guard) = test_db().await;
        let runtime = AgentRuntime::new(Arc::clone(&db));

        // No run ever registered for this id — must be a silent no-op.
        runtime
            .steer(&"never-existed".to_string(), "hello".to_string())
            .await;
        assert_eq!(runtime.active_run_count().await, 0);
        assert_eq!(
            runtime.steering_len(&"never-existed".to_string()).await,
            None
        );
    }

    // -------------------------------------------------------------------------
    // Image attachments: build_user_message (base64 -> ImageContent block).
    // -------------------------------------------------------------------------

    /// An `image/*` attachment in [`AgentRunAttachment`] shape.
    fn image_attachment(name: &str, mime: &str, data: &[u8]) -> AgentRunAttachment {
        AgentRunAttachment {
            name: name.to_string(),
            mime_type: mime.to_string(),
            data: data.to_vec(),
        }
    }

    /// VAL-RUN-018: with an image attachment present, `build_user_message`
    /// produces a content-block `UserMessage` whose first block is a
    /// `UserContentBlock::Image` carrying the base64-encoded bytes + the source
    /// mime type, followed by the text block (image first, text last).
    #[test]
    fn build_user_message_image_attachment_becomes_image_content_block() {
        let raw = [0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a]; // PNG-ish header bytes.
        let attachments = vec![image_attachment("shot.png", "image/png", &raw)];

        let msg = build_user_message("what's in this image?".to_string(), &attachments);

        let blocks = match &msg.content {
            model::UserContent::Blocks(b) => b,
            model::UserContent::Text(_) => panic!("expected content blocks, got plain text"),
        };
        assert_eq!(blocks.len(), 2, "one image block + one text block");

        // Image block first, carrying base64(data) + the source mime type.
        match &blocks[0] {
            UserContentBlock::Image(img) => {
                assert_eq!(img.data, BASE64_STANDARD.encode(raw));
                assert_eq!(img.mime_type, "image/png");
            }
            other => panic!("first block must be an image, got {other:?}"),
        }
        // Text block last, carrying the prompt.
        match &blocks[1] {
            UserContentBlock::Text(t) => assert_eq!(t.text, "what's in this image?"),
            other => panic!("second block must be the text body, got {other:?}"),
        }
    }

    /// VAL-RUN-018: a non-image attachment is skipped defensively. When it is
    /// the ONLY attachment, the message falls back to plain text (no empty
    /// content-block message).
    #[test]
    fn build_user_message_non_image_attachment_is_skipped() {
        let attachments = vec![image_attachment("notes.txt", "text/plain", b"hello")];

        let msg = build_user_message("ignore the file".to_string(), &attachments);

        match &msg.content {
            model::UserContent::Text(text) => assert_eq!(text, "ignore the file"),
            model::UserContent::Blocks(_) => {
                panic!("non-image attachment must not produce content blocks")
            }
        }
    }

    /// VAL-RUN-018: a mix of image + non-image attachments keeps only the image
    /// block(s); the non-image one is dropped, text body comes last.
    #[test]
    fn build_user_message_mixed_attachments_keeps_only_images() {
        let img = [1u8, 2, 3, 4];
        let attachments = vec![
            image_attachment("doc.pdf", "application/pdf", b"%PDF"),
            image_attachment("pic.jpg", "image/jpeg", &img),
        ];

        let msg = build_user_message("describe".to_string(), &attachments);

        let blocks = match &msg.content {
            model::UserContent::Blocks(b) => b,
            model::UserContent::Text(_) => panic!("expected content blocks"),
        };
        // 1 image (jpg) + 1 text; the pdf is dropped.
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            UserContentBlock::Image(i) => {
                assert_eq!(i.data, BASE64_STANDARD.encode(img));
                assert_eq!(i.mime_type, "image/jpeg");
            }
            other => panic!("first block must be the surviving image, got {other:?}"),
        }
        assert!(matches!(&blocks[1], UserContentBlock::Text(_)));
    }

    /// No attachments → the existing plain-text path is preserved unchanged.
    #[test]
    fn build_user_message_no_attachments_is_plain_text() {
        let msg = build_user_message("just text".to_string(), &[]);
        match &msg.content {
            model::UserContent::Text(text) => assert_eq!(text, "just text"),
            model::UserContent::Blocks(_) => panic!("text-only send must stay plain text"),
        }
    }

    /// An image attachment with an empty text body still produces an image
    /// block — and only the image block (no empty trailing text block).
    #[test]
    fn build_user_message_image_with_empty_text_emits_image_only() {
        let raw = [9u8, 9, 9];
        let attachments = vec![image_attachment("p.png", "image/png", &raw)];

        let msg = build_user_message(String::new(), &attachments);

        let blocks = match &msg.content {
            model::UserContent::Blocks(b) => b,
            model::UserContent::Text(_) => panic!("expected content blocks"),
        };
        assert_eq!(
            blocks.len(),
            1,
            "image block only; empty text is not appended"
        );
        assert!(matches!(&blocks[0], UserContentBlock::Image(_)));
    }

    /// IPC-boundary validation: an oversize image attachment (> the backend
    /// `ATTACHMENT_BYTE_CAP`) is skipped defensively, while a normal-size image
    /// in the same batch is still encoded into an Image block. The backend must
    /// not trust the frontend's 10 MiB cap.
    #[test]
    fn build_user_message_oversize_attachment_is_skipped() {
        let small = [0x89u8, 0x50, 0x4e, 0x47]; // small in-bounds image.
        let oversize = vec![0u8; ATTACHMENT_BYTE_CAP + 1];
        let attachments = vec![
            image_attachment("ok.png", "image/png", &small),
            image_attachment("huge.png", "image/png", &oversize),
        ];

        let msg = build_user_message("look".to_string(), &attachments);

        let blocks = match &msg.content {
            model::UserContent::Blocks(b) => b,
            model::UserContent::Text(_) => panic!("expected content blocks"),
        };
        // Only the small image survives + the text block; the oversize one is dropped.
        assert_eq!(blocks.len(), 2, "oversize image must be skipped");
        match &blocks[0] {
            UserContentBlock::Image(img) => {
                assert_eq!(img.data, BASE64_STANDARD.encode(small));
                assert_eq!(img.mime_type, "image/png");
            }
            other => panic!("first block must be the in-bounds image, got {other:?}"),
        }
        assert!(matches!(&blocks[1], UserContentBlock::Text(_)));
    }

    /// IPC-boundary validation: an over-count attachment list is bounded to
    /// `ATTACHMENT_MAX_COUNT` image blocks; the tail beyond the cap is skipped.
    #[test]
    fn build_user_message_over_count_attachment_list_is_bounded() {
        let raw = [1u8, 2, 3];
        let attachments: Vec<AgentRunAttachment> = (0..(ATTACHMENT_MAX_COUNT + 5))
            .map(|i| image_attachment(&format!("p{i}.png"), "image/png", &raw))
            .collect();

        // Empty text so the block count is exactly the surviving image blocks.
        let msg = build_user_message(String::new(), &attachments);

        let blocks = match &msg.content {
            model::UserContent::Blocks(b) => b,
            model::UserContent::Text(_) => panic!("expected content blocks"),
        };
        assert_eq!(
            blocks.len(),
            ATTACHMENT_MAX_COUNT,
            "attachment count is bounded to ATTACHMENT_MAX_COUNT image blocks"
        );
        assert!(blocks
            .iter()
            .all(|b| matches!(b, UserContentBlock::Image(_))));
    }

    // -------------------------------------------------------------------------
    // Skill injection at assemble time (VAL-TOOL-001/002/006/007/008/009/010/
    // 011/013/014/016/017). These drive the real `assemble_run` with a
    // `SkillService` rooted at tempdirs, then assert on the returned
    // `(context.system_prompt, tools)` — no loop spawn, no network.
    // -------------------------------------------------------------------------

    use crate::services::skill_service::SkillService;

    /// A tempdir-backed `SettingsService` (fresh default config → empty global
    /// disabled list). Returns the guard so the config dir outlives the test;
    /// the service handle lets a test toggle `skills.disabled` between
    /// assembles (the runtime re-reads the list fresh on every run).
    fn temp_settings() -> (SettingsService, TempDir) {
        let dir = TempDir::new().unwrap();
        let storage = StorageService::new(dir.path().to_path_buf()).unwrap();
        (SettingsService::new(Arc::new(storage)), dir)
    }

    /// Write `<root>/<dir>/SKILL.md` with a description, body, and optional
    /// `disable-model-invocation`. Returns the SKILL.md path.
    fn write_skill_md(
        root: &std::path::Path,
        dir: &str,
        description: &str,
        body: &str,
        opt_in: bool,
    ) {
        let skill_dir = root.join(dir);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let dmi = if opt_in {
            "\ndisable-model-invocation: true"
        } else {
            ""
        };
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\ndescription: {description}{dmi}\n---\n{body}"),
        )
        .unwrap();
    }

    /// Seed an agent session with explicit `enabled_skills`, `working_dir`, and
    /// a chosen base `system_prompt` (None → no base prompt).
    async fn seed_session_with_skills(
        db: &Arc<Database>,
        provider_id: &str,
        model_id: &str,
        enabled_skills: Vec<String>,
        enabled_tools: Vec<String>,
        working_dir: Option<String>,
        system_prompt: Option<String>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let session = AgentSession {
            id: id.clone(),
            name: "Skill Session".to_string(),
            project_id: None,
            model_id: Some(model_id.to_string()),
            provider_id: Some(provider_id.to_string()),
            system_prompt,
            thinking_level: None,
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir,
            enabled_tools,
            enabled_skills,
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

    /// Run `assemble_run` and return its `(system_prompt, tool_names)`.
    async fn assemble_prompt_and_tool_names(
        runtime: &AgentRuntime,
        session_id: &str,
    ) -> (String, Vec<String>) {
        let cancel = CancellationToken::new();
        let steering: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));
        let (_prompts, context, _config, tools) = runtime
            .assemble_run(
                &session_id.to_string(),
                "hello".to_string(),
                vec![],
                cancel,
                steering,
            )
            .await
            .expect("assemble_run should succeed");
        let names = tools.into_iter().map(|t| t.name).collect();
        (context.system_prompt, names)
    }

    /// A non-empty effective set (discovered-and-validated ∖ disabled, here
    /// with an empty disabled list) auto-injects the `skill` tool EVEN WITH
    /// `enabled_tools` empty (no read_file gate), and appends the
    /// `<available_skills>` index AFTER a non-empty base prompt.
    #[tokio::test]
    async fn nonempty_effective_set_injects_skill_tool_and_index() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let proj_skills = work.path().join(".handbox").join("skills");
        write_skill_md(
            &proj_skills,
            "alpha",
            "Alpha does things.",
            "ALPHA BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![], // no enabled_tools at all → skill tool must STILL be injected
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE PROMPT".to_string()),
        )
        .await;

        // SkillService whose app-data/user roots are empty tempdirs; the project
        // root is derived from working_dir by the service.
        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;

        assert!(
            names.contains(&"skill".to_string()),
            "skill tool injected: {names:?}"
        );
        assert!(
            prompt.contains("<available_skills>"),
            "index appended:\n{prompt}"
        );
        assert!(
            prompt.contains("<name>alpha</name>"),
            "skill listed:\n{prompt}"
        );
        // VAL-TOOL-009: base first, then index.
        let base_pos = prompt.find("BASE PROMPT").expect("base present");
        let idx_pos = prompt.find("<available_skills>").expect("index present");
        assert!(base_pos < idx_pos, "base must precede the index:\n{prompt}");
        // VAL-TOOL-014: the body must NOT appear in the system prompt.
        assert!(
            !prompt.contains("ALPHA BODY"),
            "body leaked into prompt:\n{prompt}"
        );
    }

    /// VAL-ASSEMBLE-003 (zero discovered): nothing to discover at all → no
    /// `skill` tool, no index, base prompt untouched, builtins unaffected.
    #[tokio::test]
    async fn zero_discovered_injects_nothing() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        // A working dir WITHOUT any project skills dir; app/user roots empty.
        let work = TempDir::new().unwrap();

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec!["web_fetch".to_string()],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE PROMPT".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;

        assert!(
            !names.contains(&"skill".to_string()),
            "no skill tool: {names:?}"
        );
        assert!(
            !prompt.contains("<available_skills>"),
            "no index:\n{prompt}"
        );
        // The non-skill tool (web_fetch) is still present, unaffected.
        assert!(
            names.contains(&"web_fetch".to_string()),
            "web_fetch kept: {names:?}"
        );
        // Base prompt is untouched (no trailing separator garbage).
        assert_eq!(prompt, "BASE PROMPT");
    }

    /// VAL-TOOL-008: an EMPTY base prompt produces an index-only system prompt —
    /// no leading separator/whitespace before `<available_skills>` guidance.
    #[tokio::test]
    async fn empty_base_prompt_yields_index_only_no_leading_separator() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "alpha",
            "Alpha.",
            "ALPHA BODY",
            false,
        );

        // No base prompt at all (None → unwrap_or_default → "").
        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            None,
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;

        assert!(
            names.contains(&"skill".to_string()),
            "skill tool injected: {names:?}"
        );
        // No leading newline/space: the prompt begins with the section's first
        // guidance line, not a separator.
        assert!(
            !prompt.starts_with('\n') && !prompt.starts_with(' '),
            "no leading separator before index:\n{prompt:?}"
        );
        assert!(
            prompt.starts_with("The following skills"),
            "index-only prompt begins with the section guidance:\n{prompt}"
        );
    }

    /// VAL-ASSEMBLE-028 + 002: a working dir mixing valid, broken, and globally
    /// disabled skills assembles WITHOUT aborting — valid skills are in, broken
    /// ones (validation failures) are absent, disabled ones are subtracted.
    #[tokio::test]
    async fn mixed_valid_broken_and_disabled_assembles_clean() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let proj = work.path().join(".handbox").join("skills");
        write_skill_md(&proj, "good", "Good and valid.", "GOOD BODY", false);
        write_skill_md(
            &proj,
            "muted",
            "Valid but globally disabled.",
            "MUTED BODY",
            false,
        );
        // BROKEN: frontmatter name mismatches the dir → fails validation.
        std::fs::create_dir_all(proj.join("broken-name")).unwrap();
        std::fs::write(
            proj.join("broken-name").join("SKILL.md"),
            "---\nname: different\ndescription: d\n---\nBAD BODY",
        )
        .unwrap();
        // BROKEN: no description at all → fails validation.
        std::fs::create_dir_all(proj.join("broken-desc")).unwrap();
        std::fs::write(
            proj.join("broken-desc").join("SKILL.md"),
            "---\nname: broken-desc\n---\nNO DESC BODY",
        )
        .unwrap();

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        settings.set_skill_disabled("muted", true).unwrap();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        // assemble completes (helper expects Ok) — broken skills never abort.
        let (prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;

        assert!(
            names.contains(&"skill".to_string()),
            "skill tool injected: {names:?}"
        );
        assert!(
            prompt.contains("<name>good</name>"),
            "good skill listed:\n{prompt}"
        );
        assert!(!prompt.contains("muted"), "disabled excluded:\n{prompt}");
        assert!(
            !prompt.contains("broken-name"),
            "broken (name) excluded:\n{prompt}"
        );
        assert!(
            !prompt.contains("broken-desc"),
            "broken (desc) excluded:\n{prompt}"
        );
    }

    /// Every discovered skill fails validation → empty effective set → nothing
    /// injected (and no abort).
    #[tokio::test]
    async fn all_discovered_invalid_injects_nothing() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let proj = work.path().join(".handbox").join("skills");
        // Present but invalid (no description → MissingDescription).
        std::fs::create_dir_all(proj.join("bad")).unwrap();
        std::fs::write(
            proj.join("bad").join("SKILL.md"),
            "---\nname: bad\n---\nBODY",
        )
        .unwrap();

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;
        assert!(
            !names.contains(&"skill".to_string()),
            "no skill tool: {names:?}"
        );
        assert_eq!(prompt, "BASE", "base untouched, no index");
    }

    /// VAL-ASSEMBLE-012: an opt-in (`disable-model-invocation`) skill that is
    /// not disabled still enters the effective set: it is listed with the
    /// `opt-in="true"` tag and is loadable through the injected skill tool.
    #[tokio::test]
    async fn opt_in_skill_is_listed_and_loadable() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "manual",
            "Opt-in only.",
            "MANUAL BODY",
            true, // disable-model-invocation
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(
            tools.iter().any(|t| t.name == "skill"),
            "opt-in skill still injects the tool"
        );
        assert!(
            prompt.contains("<skill opt-in=\"true\">"),
            "opt-in tag present:\n{prompt}"
        );
        assert!(
            prompt.contains("<name>manual</name>"),
            "opt-in skill listed:\n{prompt}"
        );
        // Still tool-callable: opt-in gates auto-invocation guidance, not lookup.
        assert_eq!(invoke_skill(&tools, "manual").await, "MANUAL BODY");
    }

    /// VAL-TOOL-017: a name present in multiple scopes resolves to the winning
    /// (highest-priority) scope's body via the injected skill tool. Project
    /// shadows the (lower) app-data scope.
    #[tokio::test]
    async fn shadowed_name_resolves_to_winner_body() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        // app-data scope holds a low-priority "shared"; project holds the winner.
        let app = TempDir::new().unwrap();
        write_skill_md(
            app.path(),
            "shared",
            "From app-data.",
            "APPDATA BODY",
            false,
        );

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "shared",
            "From project.",
            "PROJECT BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        // Assemble, then load "shared" through the injected skill tool's handler.
        let cancel = CancellationToken::new();
        let steering: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));
        let (_p, _ctx, _cfg, tools) = runtime
            .assemble_run(&session_id, "hi".to_string(), vec![], cancel, steering)
            .await
            .expect("assemble ok");
        let skill_tool = tools
            .into_iter()
            .find(|t| t.name == "skill")
            .expect("skill tool present");

        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc".to_string(),
            args: json!({"name": "shared"}),
            cancel: CancellationToken::new(),
            on_update: Arc::new(|_: hand_agent::ToolResult| {}),
        };
        let result = (skill_tool.execute)(ctx).await.expect("execute ok");
        let text = match &result.content[0] {
            hand_ai_model::ToolResultContent::Text(t) => t.text.clone(),
            _ => panic!("expected text result"),
        };
        assert_eq!(
            text, "PROJECT BODY",
            "winner (project) body resolved, not app-data"
        );
    }

    /// VAL-ASSEMBLE-006 + 007: the injected `skill` tool coexists with the
    /// built-in tools — builtin names (read_file / list_directory / web_fetch)
    /// are unchanged, and exactly one tool is named `skill`.
    #[tokio::test]
    async fn skill_and_builtin_tool_coexist() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        // The same working_dir is the FS sandbox root (so read_file registers) and
        // the project skills root (so alpha is discoverable).
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "alpha",
            "Alpha does things.",
            "ALPHA BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![
                "read_file".to_string(), // built-in FS tools, gated on working_dir
                "list_directory".to_string(),
                "web_fetch".to_string(),
            ],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE PROMPT".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (_prompt, names) = assemble_prompt_and_tool_names(&runtime, &session_id).await;

        for builtin in ["read_file", "list_directory", "web_fetch"] {
            assert!(
                names.contains(&builtin.to_string()),
                "built-in {builtin} survives alongside a skill: {names:?}"
            );
        }
        assert_eq!(
            names.iter().filter(|n| n.as_str() == "skill").count(),
            1,
            "exactly one skill tool alongside the builtins: {names:?}"
        );
    }

    /// Opaque `skills.disabled` entries — orphans, duplicates, empty and
    /// whitespace strings — never match a discovered name (exact match, no
    /// normalization) and never break assembly: the discovered skill is still
    /// injected once, with exactly ONE `skill` tool entry and a single body.
    #[tokio::test]
    async fn opaque_disabled_entries_never_match_and_never_break() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "foo",
            "Foo does things.",
            "FOO BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        // Orphan (twice), empty, whitespace, and a near-miss with whitespace:
        // all opaque, none may match `foo`.
        for entry in ["ghost", "", "  ", " foo "] {
            settings.set_skill_disabled(entry, true).unwrap();
        }
        settings.set_skill_disabled("ghost", true).unwrap(); // duplicate insert
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;

        // Exactly one index block for foo.
        assert_eq!(
            prompt.matches("<name>foo</name>").count(),
            1,
            "a single index block for foo:\n{prompt}"
        );
        // Exactly one skill tool entry, resolving the single body.
        assert_eq!(
            tools.iter().filter(|t| t.name == "skill").count(),
            1,
            "exactly one skill tool entry"
        );
        assert_eq!(invoke_skill(&tools, "foo").await, "FOO BODY");
    }

    // -------------------------------------------------------------------------
    // Global opt-out semantics (VAL-ASSEMBLE-001/002/003/005/006, VAL-CROSS-004):
    // effective set = discovered-and-validated ∖ settings `skills.disabled`,
    // re-read fresh from config.json on every assemble. The per-session
    // `enabled_skills` filter is GONE: assemble must not consult it.
    // -------------------------------------------------------------------------

    /// Assemble the session's run and return `(system_prompt, tools)` so tests
    /// can invoke the assembled tool handlers directly.
    async fn assemble_prompt_and_tools(
        runtime: &AgentRuntime,
        session_id: &str,
    ) -> (String, Vec<AgentTool>) {
        let cancel = CancellationToken::new();
        let steering: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(Vec::new()));
        let (_prompts, context, _config, tools) = runtime
            .assemble_run(
                &session_id.to_string(),
                "hello".to_string(),
                vec![],
                cancel,
                steering,
            )
            .await
            .expect("assemble_run should succeed");
        (context.system_prompt, tools)
    }

    /// Invoke the assembled `skill` tool with `{"name": <name>}` and return the
    /// first text block of its result (body on a hit, error message on a miss).
    async fn invoke_skill(tools: &[AgentTool], name: &str) -> String {
        let skill_tool = tools
            .iter()
            .find(|t| t.name == "skill")
            .expect("skill tool present");
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc".to_string(),
            args: json!({ "name": name }),
            cancel: CancellationToken::new(),
            on_update: Arc::new(|_: hand_agent::ToolResult| {}),
        };
        let result = (skill_tool.execute)(ctx).await.expect("execute ok");
        match &result.content[0] {
            hand_ai_model::ToolResultContent::Text(t) => t.text.clone(),
            _ => panic!("expected text result"),
        }
    }

    /// VAL-ASSEMBLE-001 + 006: with NO disabled entries, EVERY discovered-and-
    /// validated skill enters the index (name + description) and the single
    /// `skill` tool resolves each of them — with the session's `enabled_skills`
    /// left EMPTY, proving assemble no longer consults the per-session list.
    #[tokio::test]
    async fn default_all_on_injects_every_discovered_skill() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let proj = work.path().join(".handbox").join("skills");
        write_skill_md(&proj, "alpha", "Alpha does things.", "ALPHA BODY", false);
        write_skill_md(&proj, "beta", "Beta does other things.", "BETA BODY", false);

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![], // per-session enablement is dead: empty must NOT gate skills
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE PROMPT".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        // Both skills indexed with name + description.
        assert!(
            prompt.contains("<available_skills>"),
            "index present:\n{prompt}"
        );
        assert!(
            prompt.contains("<name>alpha</name>"),
            "alpha listed:\n{prompt}"
        );
        assert!(
            prompt.contains("Alpha does things."),
            "alpha description:\n{prompt}"
        );
        assert!(
            prompt.contains("<name>beta</name>"),
            "beta listed:\n{prompt}"
        );
        assert!(
            prompt.contains("Beta does other things."),
            "beta description:\n{prompt}"
        );

        // Exactly ONE tool named `skill`, resolving every indexed skill.
        assert_eq!(
            names.iter().filter(|n| **n == "skill").count(),
            1,
            "exactly one skill tool: {names:?}"
        );
        assert_eq!(invoke_skill(&tools, "alpha").await, "ALPHA BODY");
        assert_eq!(invoke_skill(&tools, "beta").await, "BETA BODY");
    }

    /// VAL-ASSEMBLE-002: a globally disabled skill is absent from the index AND
    /// from the tool's lookup map (`skill(name)` → generic not-found, no path);
    /// other skills are unaffected.
    #[tokio::test]
    async fn globally_disabled_skill_excluded_others_unaffected() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let proj = work.path().join(".handbox").join("skills");
        write_skill_md(&proj, "alpha", "Alpha does things.", "ALPHA BODY", false);
        write_skill_md(&proj, "beta", "Beta does other things.", "BETA BODY", false);

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        settings.set_skill_disabled("beta", true).unwrap();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;

        assert!(
            prompt.contains("<name>alpha</name>"),
            "alpha kept:\n{prompt}"
        );
        assert!(
            !prompt.contains("beta"),
            "disabled beta absent from index:\n{prompt}"
        );
        assert_eq!(invoke_skill(&tools, "alpha").await, "ALPHA BODY");
        let miss = invoke_skill(&tools, "beta").await;
        assert_eq!(miss, "skill not found", "disabled name is a plain miss");
        assert!(!miss.contains('/'), "no path in the miss error: {miss:?}");
        assert!(!miss.contains("BETA BODY"), "no body leak: {miss:?}");
    }

    /// VAL-ASSEMBLE-003 (all disabled): every discovered skill is globally
    /// disabled → no `<available_skills>` section, no `skill` tool, base prompt
    /// intact, unrelated builtin tools unaffected.
    #[tokio::test]
    async fn all_disabled_injects_nothing() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "alpha",
            "Alpha does things.",
            "ALPHA BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec!["web_fetch".to_string()],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE PROMPT".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        settings.set_skill_disabled("alpha", true).unwrap();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

        assert!(!names.contains(&"skill"), "no skill tool: {names:?}");
        assert!(
            !prompt.contains("<available_skills>"),
            "no index:\n{prompt}"
        );
        assert_eq!(prompt, "BASE PROMPT", "base prompt intact");
        assert!(
            names.contains(&"web_fetch"),
            "builtin unaffected: {names:?}"
        );
    }

    /// VAL-ASSEMBLE-005: the disabled list is re-read from config.json on EVERY
    /// assemble — same session, same runtime: run1 (empty list) has the skill,
    /// run2 (after disabling) lacks it, run3 (after re-enabling) has it again.
    #[tokio::test]
    async fn disabled_list_is_reread_every_assemble() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "alpha",
            "Alpha does things.",
            "ALPHA BODY",
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings.clone());

        // Run 1: empty disabled list → alpha present.
        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(
            prompt.contains("<name>alpha</name>"),
            "run1 has alpha:\n{prompt}"
        );
        assert_eq!(invoke_skill(&tools, "alpha").await, "ALPHA BODY");

        // Disable BETWEEN runs (no runtime reconstruction, same session).
        settings.set_skill_disabled("alpha", true).unwrap();

        // Run 2: the fresh read sees the disable → alpha gone, nothing injected.
        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(
            !prompt.contains("<available_skills>"),
            "run2 has no index:\n{prompt}"
        );
        assert!(
            !tools.iter().any(|t| t.name == "skill"),
            "run2 has no skill tool"
        );

        // Re-enable → run 3 has it back.
        settings.set_skill_disabled("alpha", false).unwrap();
        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(
            prompt.contains("<name>alpha</name>"),
            "run3 has alpha:\n{prompt}"
        );
        assert_eq!(invoke_skill(&tools, "alpha").await, "ALPHA BODY");
    }

    /// VAL-CROSS-004 (default all-on, end to end at the assemble layer): a
    /// freshly dropped valid SKILL.md is usable on the very next run with ZERO
    /// enablement action — config.json gains no entry for it.
    #[tokio::test]
    async fn freshly_dropped_skill_usable_next_run_without_enablement() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let work = TempDir::new().unwrap();
        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings.clone());

        // Run 1: nothing discovered → nothing injected.
        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(!prompt.contains("<available_skills>"), "run1 has no index");
        assert!(!tools.iter().any(|t| t.name == "skill"), "run1 has no tool");

        // Drop a brand-new valid skill between runs. NO settings write.
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "fresh",
            "Fresh skill.",
            "FRESH BODY",
            false,
        );

        // Run 2: discovered → indexed and callable, zero enablement action.
        let (prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        assert!(
            prompt.contains("<name>fresh</name>"),
            "run2 lists fresh:\n{prompt}"
        );
        assert_eq!(invoke_skill(&tools, "fresh").await, "FRESH BODY");
        // config.json has no entry for it (default all-on needs none).
        assert!(
            settings.get_settings().unwrap().skills.disabled.is_empty(),
            "no config entry was created for the fresh skill"
        );
    }

    /// VAL-ASSEMBLE-008 at the assemble layer: a 1MiB body flows file → map →
    /// tool verbatim and untruncated.
    #[tokio::test]
    async fn large_body_resolves_verbatim_through_assemble() {
        let (db, _guard) = test_db().await;
        let provider_id = seed_provider(&db).await;

        let big = "K".repeat(1024 * 1024);
        let work = TempDir::new().unwrap();
        write_skill_md(
            &work.path().join(".handbox").join("skills"),
            "big",
            "Big skill.",
            &big,
            false,
        );

        let session_id = seed_session_with_skills(
            &db,
            &provider_id,
            "gpt-4o",
            vec![],
            vec![],
            Some(work.path().to_string_lossy().into_owned()),
            Some("BASE".to_string()),
        )
        .await;

        let app = TempDir::new().unwrap();
        let user = TempDir::new().unwrap();
        let skills = Arc::new(SkillService::for_test(
            app.path().to_path_buf(),
            user.path().to_path_buf(),
        ));
        let (settings, _cfg) = temp_settings();
        let runtime = AgentRuntime::new_with_skills(Arc::clone(&db), skills, settings);

        let (_prompt, tools) = assemble_prompt_and_tools(&runtime, &session_id).await;
        let body = invoke_skill(&tools, "big").await;
        assert_eq!(body.len(), big.len(), "untruncated 1MiB body");
        assert_eq!(body, big, "verbatim body");
    }
}
