// chat_engine — HandBox-owned dispatch over hand_ai_model::Client.
//
// This module replaces the legacy handbox-llm chat path. After M2 every
// service in HandBox that wants to stream from an upstream LLM goes through
// `stream_chat` / `complete_chat` here; the per-provider adapters in
// `crates/handbox-llm/src/chat/*` are kept around only until M3 deletes the
// crate.
//
// The translation logic (DbMessage slice → hand-ai Context, AssistantMessageEvent
// → ChatChunk) is lifted from `crates/handbox-llm/src/chat/hand_ai_adapter.rs`
// with type renames — see the M2 row of docs/exec-plans/dissolve-handbox-llm.md.

use std::pin::Pin;
use std::sync::OnceLock;

use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use hand_ai_model::{
    self as model, AssistantMessageEvent, Client, Context, Message, SimpleStreamOptions,
    StopReason, StreamOptions, ThinkingLevel, Tool, Usage, UserMessage,
};

use crate::models::llm_types::{LlmMessageRole, ModelPricing};
use crate::models::AppError;
use crate::storage::types::model::ModelModality;
use crate::storage::types::Message as DbMessage;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Identifies the provider HandBox is calling and supplies the per-request
/// credentials. `provider_type` must match `hand_ai_model::Provider::as_str()`
/// (e.g. "openai", "anthropic", "groq").
#[derive(Debug, Clone)]
pub struct ChatProvider {
    pub provider_type: String,
    /// Override the catalog's default base URL. Empty string means "use the
    /// hand-ai Model template's base_url unchanged".
    pub base_url: String,
    pub api_key: String,
}

/// One unit of streamed assistant output, mapped from hand-ai's
/// `AssistantMessageEvent`.
#[derive(Debug, Clone, Default)]
pub struct ChatChunk {
    /// Incremental text from a `TextDelta` event.
    pub content: Option<String>,
    /// Incremental reasoning text from a `ThinkingDelta` event.
    pub reasoning: Option<String>,
    /// `"stop"` / `"length"` / `"tool_calls"` / `"content_filter"` set only on
    /// the terminal chunk (translated from `StopReason`).
    pub finish_reason: Option<String>,
    /// Token usage stats; set only on the terminal chunk.
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// Per-call options. Mirrors a subset of HandBox's chat parameter UI.
#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub tools: Vec<ChatTool>,
    /// Pass-through reasoning effort hint (e.g. "low" / "medium" / "high").
    /// Mapped to hand-ai's `SimpleStreamOptions.reasoning: Option<ThinkingLevel>`.
    pub reasoning_effort: Option<String>,
    /// External cancellation channel. None = uncancellable; Some flows into
    /// `SimpleStreamOptions.base.signal`.
    pub signal: Option<CancellationToken>,
}

#[derive(Debug, Clone)]
pub struct ChatTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Stream a chat response. The returned stream emits one `ChatChunk` per
/// non-boundary `AssistantMessageEvent`, terminating with a chunk that has
/// `finish_reason` and `usage` set. Errors during streaming become
/// `Err(AppError)` items in the stream; the stream then closes.
pub async fn stream_chat(
    provider: &ChatProvider,
    model_id: &str,
    messages: &[DbMessage],
    options: ChatOptions,
) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, AppError>> + Send>>, AppError> {
    let context = messages_to_context(messages, &provider.provider_type, model_id, &options.tools)?;
    let model = resolve_model(&provider.provider_type, model_id, &provider.base_url)?;
    let stream_options = build_stream_options(&options, &provider.api_key);

    let client = shared_client();
    let event_stream =
        hand_ai_model::stream_simple(&client.registry, &model, context, Some(stream_options))
            .map_err(client_err_to_app_err)?;

    let chunk_stream =
        event_stream.filter_map(|event| async move { event_to_chunk_result(&event) });

    Ok(Box::pin(chunk_stream))
}

/// Non-stream variant: collect the full response into a single `ChatChunk`
/// whose `content` is the concatenated text. Used by `services/session.rs`
/// for title generation. Internally drives `stream_chat` and folds.
pub async fn complete_chat(
    provider: &ChatProvider,
    model_id: &str,
    messages: &[DbMessage],
    options: ChatOptions,
) -> Result<ChatChunk, AppError> {
    let mut stream = stream_chat(provider, model_id, messages, options).await?;

    let mut acc = ChatChunk::default();
    let mut content = String::new();
    let mut reasoning = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        if let Some(c) = chunk.content {
            content.push_str(&c);
        }
        if let Some(r) = chunk.reasoning {
            reasoning.push_str(&r);
        }
        if chunk.finish_reason.is_some() {
            acc.finish_reason = chunk.finish_reason;
        }
        if chunk.usage.is_some() {
            acc.usage = chunk.usage;
        }
    }

    if !content.is_empty() {
        acc.content = Some(content);
    }
    if !reasoning.is_empty() {
        acc.reasoning = Some(reasoning);
    }
    Ok(acc)
}

/// List the models hand-ai knows about under `provider_type`. Maps each
/// `hand_ai_model::Model` to HandBox's `storage::types::Model`. Used by
/// `services/model.rs` to refresh the model picker without going through
/// the per-protocol `/v1/models` endpoint.
pub fn list_catalog_models(provider_type: &str) -> Vec<crate::storage::types::Model> {
    let now = chrono::Utc::now().timestamp_millis();
    hand_ai_model::get_models(provider_type)
        .into_iter()
        .map(|m| hand_ai_to_handbox_model(provider_type, &m, now))
        .collect()
}

// ---------------------------------------------------------------------------
// Shared Client (cheap to clone, but the inner Arc<ApiProviderRegistry> only
// needs to be built once per process).
// ---------------------------------------------------------------------------

fn shared_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::new)
}

// ---------------------------------------------------------------------------
// Translation: &[DbMessage] → hand_ai_model::Context
// ---------------------------------------------------------------------------

/// Convert a HandBox `DbMessage` slice into a hand-ai `Context`.
///
/// HandBox encodes the system prompt as `LlmMessageRole::System` rows in
/// `messages`; hand-ai expects it on `Context::system_prompt`. Multiple
/// system messages are concatenated with a blank line between them — same
/// behaviour the legacy `hand_ai_adapter` uses for the request path.
pub(crate) fn messages_to_context(
    messages: &[DbMessage],
    provider_id: &str,
    model_id: &str,
    tools: &[ChatTool],
) -> Result<Context, AppError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut out: Vec<Message> = Vec::with_capacity(messages.len());
    // Threaded so role=Tool entries can recover the tool name from the
    // preceding role=Assistant turn that declared the call. HandBox's
    // DbMessage(role=Tool) only carries `tool_call_id`; hand-ai requires
    // `tool_name` on ToolResultMessage.
    let mut tool_name_by_call_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // For prior assistant turns, hand-ai's AssistantMessage requires
    // api/provider/model metadata HandBox doesn't store per-message. Best-effort
    // reconstruct from the *current* request's model: even if a past turn ran
    // on a different model, the historical record is about *content*, not
    // provenance — and hand-ai's transcript handling doesn't validate prior
    // turns against the current model. If the current model id isn't in
    // hand-ai's catalog we surface a clear error rather than fabricate metadata.
    let assistant_meta = if messages.iter().any(|m| m.role == LlmMessageRole::Assistant) {
        let template = hand_ai_model::get_model(provider_id, model_id).ok_or_else(|| {
            AppError::validation_error(&format!(
                "chat_engine: cannot reconstruct prior assistant turn — model '{}' under provider '{}' not in catalog",
                model_id, provider_id
            ))
        })?;
        Some(AssistantMeta {
            api: template.api,
            provider: template.provider,
            model: template.id,
        })
    } else {
        None
    };

    for msg in messages {
        match msg.role {
            LlmMessageRole::System => {
                if !msg.content.is_empty() {
                    system_chunks.push(msg.content.clone());
                }
            }
            LlmMessageRole::User => {
                out.push(Message::User(db_user_message(msg)));
            }
            LlmMessageRole::Assistant => {
                if let Some(calls) = msg.tool_calls.as_ref() {
                    for c in calls {
                        tool_name_by_call_id.insert(c.id.clone(), c.function.name.clone());
                    }
                }
                let meta = assistant_meta
                    .as_ref()
                    .expect("computed when any assistant entry exists");
                out.push(Message::Assistant(db_assistant_message(msg, meta)?));
            }
            LlmMessageRole::Tool => {
                out.push(Message::ToolResult(db_tool_result_message(
                    msg,
                    &tool_name_by_call_id,
                )?));
            }
        }
    }

    let system_prompt = if system_chunks.is_empty() {
        None
    } else {
        Some(system_chunks.join("\n\n"))
    };

    let tools = if tools.is_empty() {
        None
    } else {
        Some(translate_tools(tools))
    };

    Ok(Context {
        system_prompt,
        messages: out,
        tools,
    })
}

/// api / provider / model triple recovered from the current request's
/// model, used to fill required fields on reconstructed prior assistant
/// turns. Cached once per request so multi-turn histories don't pay the
/// catalog lookup per message.
struct AssistantMeta {
    api: model::Api,
    provider: hand_ai_model::types::Provider,
    model: String,
}

fn db_user_message(msg: &DbMessage) -> UserMessage {
    // Attachments on DbMessage carry a filesystem `path`, not raw bytes; the
    // legacy adapter loads bytes upstream and packs an `UserContentBlock::Image`.
    // M2-T1 only owns the dispatch shell — attachment hydration stays at the
    // caller (or moves into chat_engine later). For now, treat the message as
    // plain text; the caller that wants image attachments must pass them in a
    // pre-translated representation in a follow-up task.
    UserMessage::new_text(msg.content.clone())
}

fn db_assistant_message(
    msg: &DbMessage,
    meta: &AssistantMeta,
) -> Result<model::AssistantMessage, AppError> {
    let mut content: Vec<model::AssistantContentBlock> = Vec::new();

    // Reasoning → ThinkingContent. Preserve so the next turn sees the same
    // context the user did.
    if let Some(reasoning) = msg.reasoning.as_ref().filter(|s| !s.is_empty()) {
        content.push(model::AssistantContentBlock::Thinking(
            model::ThinkingContent::new(reasoning.clone()),
        ));
    }

    if !msg.content.is_empty() {
        content.push(model::AssistantContentBlock::Text(model::TextContent::new(
            msg.content.clone(),
        )));
    }

    if let Some(calls) = msg.tool_calls.as_ref() {
        for call in calls {
            // HandBox stores arguments as a JSON-encoded string; hand-ai's
            // ToolCall.arguments is a Value.
            let args: serde_json::Value = if call.function.arguments.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_json::from_str(&call.function.arguments).map_err(|e| {
                    AppError::validation_error(&format!(
                        "chat_engine: tool_call arguments not valid JSON for call '{}': {}",
                        call.id, e
                    ))
                })?
            };
            content.push(model::AssistantContentBlock::ToolCall(
                model::ToolCall::new(call.id.clone(), call.function.name.clone(), args),
            ));
        }
    }

    Ok(model::AssistantMessage {
        role: "assistant".to_string(),
        content,
        api: meta.api,
        provider: meta.provider,
        model: meta.model.clone(),
        usage: model::Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0,
        response_model: None,
        response_id: None,
        diagnostics: None,
    })
}

fn db_tool_result_message(
    msg: &DbMessage,
    tool_name_by_call_id: &std::collections::HashMap<String, String>,
) -> Result<model::ToolResultMessage, AppError> {
    let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
        AppError::validation_error("chat_engine: DbMessage role=Tool requires tool_call_id")
    })?;
    let tool_name = tool_name_by_call_id.get(tool_call_id).ok_or_else(|| {
        AppError::validation_error(&format!(
            "chat_engine: tool result for unknown tool_call_id '{}' (no preceding assistant tool_call with this id)",
            tool_call_id
        ))
    })?;
    Ok(model::ToolResultMessage {
        role: "toolResult".to_string(),
        tool_call_id: tool_call_id.clone(),
        tool_name: tool_name.clone(),
        content: vec![model::ToolResultContent::Text(model::TextContent::new(
            msg.content.clone(),
        ))],
        details: None,
        is_error: false,
        timestamp: 0,
    })
}

fn translate_tools(tools: &[ChatTool]) -> Vec<Tool> {
    tools
        .iter()
        .map(|t| Tool::new(t.name.clone(), t.description.clone(), t.parameters.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// Model lookup + options building
// ---------------------------------------------------------------------------

/// Look up the hand-ai `Model` template and override `base_url` from the
/// caller-supplied `ChatProvider`.
fn resolve_model(
    provider_id: &str,
    model_id: &str,
    base_url: &str,
) -> Result<model::Model, AppError> {
    let mut m = hand_ai_model::get_model(provider_id, model_id).ok_or_else(|| {
        AppError::validation_error(&format!(
            "chat_engine: model '{}' not registered under provider '{}'",
            model_id, provider_id
        ))
    })?;
    if !base_url.is_empty() {
        m.base_url = base_url.to_string();
    }
    Ok(m)
}

#[allow(clippy::field_reassign_with_default)]
// StreamOptions and SimpleStreamOptions are #[non_exhaustive] in hand_ai_model
// (since #32 / commit 7994163). FRU (`..Default::default()`) is illegal from
// outside the defining crate, so we mutate-default.
fn build_stream_options(options: &ChatOptions, api_key: &str) -> SimpleStreamOptions {
    let mut base = StreamOptions::default();
    base.api_key = Some(api_key.to_string());
    base.temperature = options.temperature;
    base.max_tokens = options.max_tokens;
    base.signal = options.signal.clone();
    let mut opts = SimpleStreamOptions::default();
    opts.base = base;
    opts.reasoning = options
        .reasoning_effort
        .as_deref()
        .and_then(parse_thinking_level);
    opts
}

fn parse_thinking_level(s: &str) -> Option<ThinkingLevel> {
    match s.to_ascii_lowercase().as_str() {
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "xhigh" | "x-high" | "extra-high" => Some(ThinkingLevel::Xhigh),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Translation: AssistantMessageEvent → ChatChunk
// ---------------------------------------------------------------------------

/// Map hand-ai's `ClientError` onto HandBox's `AppError`. Routed per variant
/// so OAuth re-init / config-validation / unexpected-internal paths land on
/// the right HandBox error code:
///
/// - `OAuthRequired` → `AppError::auth_error` (triggers provider OAuth re-init
///   in the UI; collapsing this to `validation_error` would surface
///   "请检查输入参数" instead of an auth prompt).
/// - `ProviderNotFound` → `AppError::validation_error` (model/provider pair
///   isn't in hand-ai's static catalog — caller-side config issue).
/// - `StreamEndedWithoutResult` → `AppError::internal_error` (no terminal
///   event from the provider stream; not a user-correctable failure).
fn client_err_to_app_err(err: hand_ai_model::ClientError) -> AppError {
    use hand_ai_model::ClientError;
    match &err {
        ClientError::OAuthRequired { .. } => AppError::auth_error(&format!("{}", err)),
        ClientError::ProviderNotFound { .. } => {
            AppError::validation_error(&format!("hand-ai client error: {}", err))
        }
        ClientError::StreamEndedWithoutResult => {
            AppError::internal_error(&format!("hand-ai client error: {}", err))
        }
    }
}

fn usage_to_chat(u: &Usage) -> Option<ChatUsage> {
    if u.input == 0 && u.output == 0 && u.total_tokens == 0 {
        return None;
    }
    Some(ChatUsage {
        prompt_tokens: i32::try_from(u.input).unwrap_or(i32::MAX),
        completion_tokens: i32::try_from(u.output).unwrap_or(i32::MAX),
        total_tokens: i32::try_from(u.total_tokens).unwrap_or(i32::MAX),
    })
}

/// Map hand-ai's terminal `StopReason` to a HandBox `finish_reason` string.
fn stop_reason_to_finish_reason(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Stop => "stop",
        StopReason::Length => "length",
        StopReason::ToolUse => "tool_calls",
        StopReason::Aborted => "stop",
        // `Error` is propagated as `AppError` upstream of this fn; if it ever
        // reaches here, surface as a plain stop rather than panic.
        StopReason::Error => "stop",
    }
}

/// Translate one hand-ai event to one HandBox chunk, if applicable.
///
/// `*_Start` / `*_End` boundary variants emit nothing (the matching `_Delta`
/// events carry the payload). `Error` is translated to `Err` so the consumer's
/// stream terminates with the failure rather than receiving a synthetic chunk.
/// `ToolCall*` variants are intentionally dropped for M2 — tool-call streaming
/// has its own aggregation semantics that need a dedicated translation block
/// (acknowledged gap in the dissolve-handbox-llm plan).
pub(crate) fn event_to_chunk_result(
    event: &AssistantMessageEvent,
) -> Option<Result<ChatChunk, AppError>> {
    match event {
        AssistantMessageEvent::TextDelta { delta, .. } => Some(Ok(ChatChunk {
            content: Some(delta.clone()),
            ..Default::default()
        })),
        AssistantMessageEvent::ThinkingDelta { delta, .. } => Some(Ok(ChatChunk {
            reasoning: Some(delta.clone()),
            ..Default::default()
        })),
        AssistantMessageEvent::Done { reason, message } => Some(Ok(ChatChunk {
            finish_reason: Some(stop_reason_to_finish_reason(reason).to_string()),
            usage: usage_to_chat(&message.usage),
            ..Default::default()
        })),
        AssistantMessageEvent::Error { error, .. } => {
            let msg = error
                .error_message
                .clone()
                .unwrap_or_else(|| "hand-ai stream returned Error event".to_string());
            Some(Err(AppError::validation_error(&msg)))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Translation: hand_ai_model::Model → storage::types::Model
// ---------------------------------------------------------------------------

fn hand_ai_to_handbox_model(
    provider_id: &str,
    m: &model::Model,
    now: i64,
) -> crate::storage::types::Model {
    let input_modalities: Vec<ModelModality> = m
        .input
        .iter()
        .map(|i| match i {
            model::InputType::Text => ModelModality::Text,
            model::InputType::Image => ModelModality::Image,
        })
        .collect();
    // hand-ai's Model only describes input modalities; outputs are implicitly
    // text for every chat-completion-style API.
    let output_modalities = vec![ModelModality::Text];

    let context_length = i32::try_from(m.context_window).ok();
    let output_max_tokens = i32::try_from(m.max_tokens).ok();

    let pricing = Some(ModelPricing {
        currency: Some("USD".to_string()),
        input_text: Some(m.cost.input as f32),
        output_text: Some(m.cost.output as f32),
    });

    crate::storage::types::Model {
        id: m.id.clone(),
        provider_id: provider_id.to_string(),
        name: m.name.clone(),
        context_length,
        output_max_tokens,
        supported_features: None,
        description: None,
        input_modalities: Some(input_modalities),
        output_modalities: Some(output_modalities),
        metadata: None,
        pricing,
        url: None,
        supported_parameters: None,
        default_parameters: None,
        max_parameters: None,
        supported_methods: None,
        model_created_at: None,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::llm_types::LlmToolFunction;
    use crate::storage::types::{
        Message as DbMessage, MessageToolCall, MessageToolExecutionMode, MessageToolExecutionStatus,
    };

    fn bare_db_message(role: LlmMessageRole, content: &str) -> DbMessage {
        DbMessage {
            id: "msg".to_string(),
            session_id: "sess".to_string(),
            role,
            content: content.to_string(),
            reasoning: None,
            tool_calls: None,
            turn_id: None,
            tool_call_id: None,
            config: None,
            attachments: None,
            generated_assets: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn tool_call(id: &str, name: &str, arguments: &str) -> MessageToolCall {
        MessageToolCall {
            id: id.to_string(),
            tool_type: "function".to_string(),
            function: LlmToolFunction {
                name: name.to_string(),
                arguments: arguments.to_string(),
            },
            execution_mode: MessageToolExecutionMode::default(),
            execution_status: MessageToolExecutionStatus::default(),
            result: None,
        }
    }

    fn partial_assistant() -> model::AssistantMessage {
        model::AssistantMessage {
            role: "assistant".into(),
            content: vec![],
            api: model::Api::OpenAICompletions,
            provider: hand_ai_model::types::Provider::OpenAI,
            model: "gpt-4o".into(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        }
    }

    #[test]
    fn builds_context_from_user_message() {
        let messages = vec![bare_db_message(LlmMessageRole::User, "hello")];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &[]).expect("translation");
        assert!(ctx.system_prompt.is_none());
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn extracts_system_prompt_from_leading_system_message() {
        let messages = vec![
            bare_db_message(LlmMessageRole::System, "be helpful"),
            bare_db_message(LlmMessageRole::User, "hi"),
        ];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &[]).expect("translation");
        assert_eq!(ctx.system_prompt.as_deref(), Some("be helpful"));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn text_delta_event_maps_to_content_chunk() {
        let event = AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hi".into(),
            partial: partial_assistant(),
        };
        let chunk = event_to_chunk_result(&event)
            .expect("event maps")
            .expect("ok variant");
        assert_eq!(chunk.content.as_deref(), Some("hi"));
        assert!(chunk.reasoning.is_none());
        assert!(chunk.finish_reason.is_none());
        assert!(chunk.usage.is_none());
    }

    #[test]
    fn done_event_maps_to_terminal_chunk_with_finish_reason() {
        let mut msg = partial_assistant();
        msg.usage = Usage {
            input: 5,
            output: 10,
            cache_read: 0,
            cache_write: 0,
            total_tokens: 15,
            cost: Default::default(),
        };
        let event = AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message: msg,
        };
        let chunk = event_to_chunk_result(&event)
            .expect("event maps")
            .expect("ok variant");
        assert_eq!(chunk.finish_reason.as_deref(), Some("stop"));
        let usage = chunk.usage.expect("usage forwarded");
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 10);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn assistant_history_errors_when_model_not_in_catalog() {
        let messages = vec![bare_db_message(LlmMessageRole::Assistant, "prior reply")];
        let err = messages_to_context(&messages, "openai", "no-such-model-9999", &[])
            .expect_err("unknown model must surface clearly");
        let m = format!("{}", err);
        assert!(m.contains("not in catalog"), "msg: {m}");
        assert!(m.contains("no-such-model-9999"), "msg: {m}");
    }

    #[test]
    fn tool_result_pulls_tool_name_from_prior_assistant_call() {
        let mut assistant = bare_db_message(LlmMessageRole::Assistant, "");
        assistant.tool_calls = Some(vec![tool_call("call_99", "search_web", "{}")]);
        let mut tool_result = bare_db_message(LlmMessageRole::Tool, "result text");
        tool_result.tool_call_id = Some("call_99".to_string());

        let messages = vec![assistant, tool_result];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &[]).expect("translation");
        match &ctx.messages[1] {
            Message::ToolResult(tr) => {
                assert_eq!(tr.tool_call_id, "call_99");
                assert_eq!(tr.tool_name, "search_web");
                match &tr.content[..] {
                    [model::ToolResultContent::Text(t)] => assert_eq!(t.text, "result text"),
                    other => panic!("expected text result, got {:?}", other),
                }
            }
            other => panic!("expected tool result, got {:?}", other),
        }
    }

    #[test]
    fn tool_result_with_unknown_call_id_errors() {
        let mut orphan = bare_db_message(LlmMessageRole::Tool, "orphan");
        orphan.tool_call_id = Some("ghost".to_string());
        let messages = vec![orphan];
        let err = messages_to_context(&messages, "openai", "gpt-4o", &[])
            .expect_err("orphan tool result should error");
        assert!(format!("{}", err).contains("unknown tool_call_id"));
    }

    #[test]
    fn assistant_history_preserves_reasoning_and_tool_calls() {
        let mut assistant = bare_db_message(LlmMessageRole::Assistant, "calling tool");
        assistant.reasoning = Some("considering options".to_string());
        assistant.tool_calls = Some(vec![tool_call("call_1", "get_weather", r#"{"city":"sf"}"#)]);

        let messages = vec![assistant];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &[]).expect("translation");
        match &ctx.messages[0] {
            Message::Assistant(am) => {
                assert_eq!(am.content.len(), 3, "thinking + text + tool_call");
                assert!(matches!(
                    am.content[0],
                    model::AssistantContentBlock::Thinking(_)
                ));
                assert!(matches!(
                    am.content[1],
                    model::AssistantContentBlock::Text(_)
                ));
                match &am.content[2] {
                    model::AssistantContentBlock::ToolCall(tc) => {
                        assert_eq!(tc.id, "call_1");
                        assert_eq!(tc.name, "get_weather");
                        assert_eq!(tc.arguments["city"], "sf");
                    }
                    other => panic!("expected ToolCall, got {:?}", other),
                }
            }
            other => panic!("expected assistant, got {:?}", other),
        }
    }

    #[test]
    fn multiple_system_messages_concatenated_with_blank_line() {
        let messages = vec![
            bare_db_message(LlmMessageRole::System, "first"),
            bare_db_message(LlmMessageRole::User, "hi"),
            bare_db_message(LlmMessageRole::System, "second"),
        ];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &[]).expect("translation");
        assert_eq!(ctx.system_prompt.as_deref(), Some("first\n\nsecond"));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn error_event_translates_to_err() {
        let mut msg = partial_assistant();
        msg.error_message = Some("upstream blew up".to_string());
        let event = AssistantMessageEvent::Error {
            reason: StopReason::Error,
            error: msg,
        };
        let result = event_to_chunk_result(&event).expect("event maps");
        let err = result.expect_err("error event must surface as Err");
        assert!(format!("{}", err).contains("upstream blew up"));
    }
}
