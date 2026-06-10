// chat_engine — HandBox-owned dispatch over hand_ai_model::Client.
//
// Every HandBox service that streams from an upstream LLM goes through
// `stream_chat` / `complete_chat` here. The translation logic (ChatMessage
// slice → hand-ai Context, AssistantMessageEvent → ChatChunk) was lifted
// from the now-deleted handbox-llm chat adapter — see the M2 row of
// docs/exec-plans/dissolve-handbox-llm.md for the migration history.
//
// M2-T2a expanded the surface area: `ChatMessage` / `ChatToolCall` /
// `HydratedAttachment` are HandBox-app-internal carrier types that keep
// chat_engine storage-layer agnostic. Service callers translate from their
// richer `MessageRequest` / `Message` representations at the chat_engine
// boundary (wired up in M2-T2b/c).

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::OnceLock;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;

use hand_ai_model::{
    self as model, AssistantMessageEvent, Client, Context, ImageContent, Message,
    SimpleStreamOptions, StopReason, StreamOptions, TextContent, ThinkingLevel, Tool, Usage,
    UserContentBlock, UserMessage,
};

use crate::models::llm_types::{LlmMessageRole, ModelPricing};
use crate::models::AppError;
use crate::storage::types::model::ModelModality;

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

/// A tool call surfaced from the terminal assistant message. Mirrors
/// HandBox's storage-layer MessageToolCall shape for downstream consumers,
/// but kept here as a HandBox-app-internal type so chat_engine stays
/// storage-layer agnostic.
#[derive(Debug, Clone)]
pub struct ChatToolCall {
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments (mirrors handbox-llm's `LlmToolFunction.arguments`
    /// shape; downstream consumers do not need to re-parse).
    pub arguments: String,
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
    /// Tool calls harvested from the terminal `Done` event's assistant
    /// message content. Populated only on the terminal chunk.
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

#[derive(Debug, Clone, Default)]
pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// One message in a chat context as `chat_engine` sees it. Lighter than
/// `storage::types::Message` — only the fields actually used in translation.
/// Service-layer callers translate from their richer `MessageRequest` /
/// `Message` representations at the chat_engine boundary.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Stable per-message id used to key into `ChatOptions::hydrated_attachments`.
    /// Service callers may use the DB message id, the in-request synthetic id,
    /// or any other stable string — uniqueness is the only requirement.
    pub id: String,
    pub role: LlmMessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
    pub tool_call_id: Option<String>,
    /// Presence indicator for hydrated attachments. Non-empty means
    /// `ChatOptions::hydrated_attachments` must carry an entry under this
    /// message's `id`. The individual ids carried here are informational
    /// only — the keying is by **message id**, see the docstring on
    /// `ChatOptions::hydrated_attachments`. Empty Vec means "no attachments".
    pub attachment_ids: Vec<String>,
}

/// Pre-loaded attachment payload. Hand-ai's `UserContentBlock::Image` needs
/// raw bytes + mime; HandBox's storage stores file paths, so the service
/// layer hydrates before calling `stream_chat` / `complete_chat`.
#[derive(Debug, Clone)]
pub struct HydratedAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
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
    /// Service callers pre-load attachment bytes **keyed by message id**.
    /// When a `ChatMessage` has non-empty `attachment_ids`, chat_engine looks
    /// up the hydrated payloads here under the message's own `id` (the
    /// `attachment_ids` vec is a presence indicator only — its individual
    /// values are not used as keys). Missing entries cause the attachment
    /// to be silently dropped with a `tracing::warn!` log line (no failure).
    pub hydrated_attachments: HashMap<String, Vec<HydratedAttachment>>,
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
    messages: &[ChatMessage],
    options: ChatOptions,
) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, AppError>> + Send>>, AppError> {
    let context = messages_to_context(messages, &provider.provider_type, model_id, &options)?;
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
    messages: &[ChatMessage],
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
        // Tool calls only land on the terminal chunk; just take the last
        // non-None set, which is necessarily the terminal one.
        if chunk.tool_calls.is_some() {
            acc.tool_calls = chunk.tool_calls;
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

pub(crate) fn shared_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::new)
}

// ---------------------------------------------------------------------------
// Translation: &[ChatMessage] → hand_ai_model::Context
// ---------------------------------------------------------------------------

/// Convert a slice of `ChatMessage` into a hand-ai `Context`.
///
/// HandBox encodes the system prompt as `LlmMessageRole::System` rows in
/// `messages`; hand-ai expects it on `Context::system_prompt`. Multiple
/// system messages are concatenated with a blank line between them — same
/// behaviour the legacy `hand_ai_adapter` uses for the request path.
///
/// `model_id` is consulted when any prior assistant turn is present (hand-ai
/// requires api/provider/model metadata on `AssistantMessage`); an unknown
/// model id surfaces as a validation error in that case. `options.hydrated_attachments`
/// is consulted while translating user messages that carry non-empty
/// `attachment_ids`.
pub(crate) fn messages_to_context(
    messages: &[ChatMessage],
    provider_id: &str,
    model_id: &str,
    options: &ChatOptions,
) -> Result<Context, AppError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut out: Vec<Message> = Vec::with_capacity(messages.len());
    // Threaded so role=Tool entries can recover the tool name from the
    // preceding role=Assistant turn that declared the call. HandBox's
    // ChatMessage(role=Tool) only carries `tool_call_id`; hand-ai requires
    // `tool_name` on ToolResultMessage.
    let mut tool_name_by_call_id: HashMap<String, String> = HashMap::new();

    // For prior assistant turns, hand-ai's AssistantMessage requires
    // api/provider/model metadata HandBox doesn't store per-message. Best-effort
    // reconstruct from the *current* request's model: even if a past turn ran
    // on a different model, the historical record is about *content*, not
    // provenance — and hand-ai's transcript handling doesn't validate prior
    // turns against the current model. Catalog models resolve from hand-ai;
    // custom-provider models synthesize a template (same path as the chat
    // request). Only a non-catalog, non-custom model surfaces a clear error.
    let assistant_meta = if messages.iter().any(|m| m.role == LlmMessageRole::Assistant) {
        let template = resolve_model_template(provider_id, model_id)?;
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
                out.push(Message::User(chat_user_message(
                    msg,
                    &options.hydrated_attachments,
                )));
            }
            LlmMessageRole::Assistant => {
                if let Some(calls) = msg.tool_calls.as_ref() {
                    for c in calls {
                        tool_name_by_call_id.insert(c.id.clone(), c.name.clone());
                    }
                }
                let meta = assistant_meta.as_ref().ok_or_else(|| {
                    AppError::internal_error(
                        "chat_engine: assistant_meta missing despite assistant entry present — invariant broken",
                    )
                })?;
                out.push(Message::Assistant(chat_assistant_message(msg, meta)?));
            }
            LlmMessageRole::Tool => {
                out.push(Message::ToolResult(chat_tool_result_message(
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

    let tools = if options.tools.is_empty() {
        None
    } else {
        Some(translate_tools(&options.tools))
    };

    Ok(Context {
        system_prompt,
        messages: out,
        tools,
    })
}

/// api / provider / model triple recovered from the current request's model,
/// used to fill required fields on reconstructed prior assistant turns.
/// Cached once per request so multi-turn histories don't pay the catalog
/// lookup per message.
struct AssistantMeta {
    api: model::Api,
    provider: hand_ai_model::types::Provider,
    model: String,
}

/// Build a hand-ai `UserMessage` from a `ChatMessage`, hydrating any
/// attachments referenced by the message's id.
///
/// When `attachment_ids` is empty the message becomes a plain
/// `UserMessage::new_text`. When non-empty, `hydrated_attachments` is looked
/// up under `msg.id`:
///
/// - **Missing entry**: log a `tracing::warn!` and fall back to text-only.
/// - **Non-image attachment**: log a `tracing::warn!` and drop it (does not
///   error — by design, callers handle non-image surfaces upstream).
/// - **Image attachment**: emit one `UserContentBlock::Image` (base64-encoded
///   per hand-ai's expectation).
///
/// Order matches the legacy adapter: image blocks first, the text body last
/// (if non-empty).
fn chat_user_message(
    msg: &ChatMessage,
    hydrated_attachments: &HashMap<String, Vec<HydratedAttachment>>,
) -> UserMessage {
    if msg.attachment_ids.is_empty() {
        return UserMessage::new_text(msg.content.clone());
    }

    let Some(attachments) = hydrated_attachments.get(&msg.id).filter(|a| !a.is_empty()) else {
        tracing::warn!(
            message_id = %msg.id,
            attachment_count = msg.attachment_ids.len(),
            "chat_engine: message declares attachments but no hydrated payload found; sending text only"
        );
        return UserMessage::new_text(msg.content.clone());
    };

    let mut blocks: Vec<UserContentBlock> = Vec::with_capacity(attachments.len() + 1);
    for att in attachments {
        if !att.mime_type.starts_with("image/") {
            tracing::warn!(
                message_id = %msg.id,
                attachment_name = %att.name,
                mime_type = %att.mime_type,
                "chat_engine: non-image attachment dropped (only image/* supported in this path)"
            );
            continue;
        }
        let data_b64 = BASE64_STANDARD.encode(&att.data);
        blocks.push(UserContentBlock::Image(ImageContent::new(
            data_b64,
            att.mime_type.clone(),
        )));
    }

    if blocks.is_empty() {
        // All attachments were dropped above — fall back to text-only.
        return UserMessage::new_text(msg.content.clone());
    }

    if !msg.content.is_empty() {
        blocks.push(UserContentBlock::Text(TextContent::new(msg.content.clone())));
    }
    UserMessage::new_blocks(blocks)
}

fn chat_assistant_message(
    msg: &ChatMessage,
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
            // `ChatToolCall::arguments` is JSON-encoded text mirroring
            // handbox-llm's LlmToolFunction shape; hand-ai's ToolCall.arguments
            // is a Value.
            let args: serde_json::Value = if call.arguments.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_json::from_str(&call.arguments).map_err(|e| {
                    AppError::validation_error(&format!(
                        "chat_engine: tool_call arguments not valid JSON for call '{}': {}",
                        call.id, e
                    ))
                })?
            };
            content.push(model::AssistantContentBlock::ToolCall(
                model::ToolCall::new(call.id.clone(), call.name.clone(), args),
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

fn chat_tool_result_message(
    msg: &ChatMessage,
    tool_name_by_call_id: &HashMap<String, String>,
) -> Result<model::ToolResultMessage, AppError> {
    let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
        AppError::validation_error("chat_engine: ChatMessage role=Tool requires tool_call_id")
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

/// Resolve a `Model` template by `provider_id` (== provider_type) + `model_id`.
///
/// Catalog providers resolve from hand-ai's static catalog. Custom providers
/// (openai-compatible / anthropic-compatible) aren't catalog entries, so a
/// template is synthesized from the wire protocol the custom type speaks (the
/// user supplies the model id + base_url). Anything else is a real error.
fn resolve_model_template(provider_id: &str, model_id: &str) -> Result<model::Model, AppError> {
    if let Some(m) = hand_ai_model::get_model(provider_id, model_id) {
        return Ok(m);
    }
    // Fallback: the model picker is populated from `get_models()` (the catalog
    // list, see `list_catalog_models`), which for some catalog providers can
    // include entries that `get_model()` won't resolve by id — notably OpenRouter
    // ":free" variants (e.g. `deepseek/deepseek-v4-flash:free`). Searching the
    // same list the picker uses guarantees that anything offered is actually
    // runnable, using the catalog's own metadata (api / reasoning / cost) rather
    // than a synthesized stand-in. Fixes Chat + Agent identically.
    if let Some(m) = hand_ai_model::get_models(provider_id)
        .into_iter()
        .find(|m| m.id.as_str() == model_id)
    {
        return Ok(m);
    }
    if let Some(api) = custom_api_for_provider_type(provider_id) {
        return Ok(synthesize_custom_model(model_id, api));
    }
    Err(AppError::validation_error(&format!(
        "chat_engine: model '{}' not registered under provider '{}'",
        model_id, provider_id
    )))
}

/// Resolve the model template and override `base_url` from the caller-supplied
/// `ChatProvider` (mandatory for custom providers — the synthesized template
/// has no endpoint of its own).
pub(crate) fn resolve_model(
    provider_id: &str,
    model_id: &str,
    base_url: &str,
) -> Result<model::Model, AppError> {
    let mut m = resolve_model_template(provider_id, model_id)?;
    if !base_url.is_empty() {
        m.base_url = base_url.to_string();
    }
    Ok(m)
}

/// Map a HandBox custom-provider type to the hand-ai wire protocol it speaks.
///
/// Custom providers are HandBox-owned onboarding templates for unlisted
/// OpenAI-/Anthropic-compatible endpoints (local LLMs, proxies, vendors not in
/// hand-ai's catalog). hand-ai can't know about them, so HandBox owns this
/// fixed mapping. Returns `None` for catalog provider types.
pub(crate) fn custom_api_for_provider_type(provider_type: &str) -> Option<model::Api> {
    match provider_type {
        "openai-compatible" => Some(model::Api::OpenAICompletions),
        "anthropic-compatible" => Some(model::Api::AnthropicMessages),
        _ => None,
    }
}

/// The chat-method tags a manually-added model under a custom provider should
/// carry so it renders in the picker. `None` for non-custom provider types.
pub fn custom_provider_supported_methods(provider_type: &str) -> Option<Vec<String>> {
    custom_api_for_provider_type(provider_type).map(supported_methods_for_api)
}

/// Build a minimal `Model` template for a custom-provider model that isn't in
/// hand-ai's catalog. Stream dispatch keys off `api` (client.rs
/// `registry.get(&model.api)`), so only `api` + `base_url` (filled by the
/// caller) are load-bearing. `provider` is metadata only — it feeds the env-key
/// fallback (we always pass an explicit key) and a GitHubCopilot special-case
/// (avoided here), so a same-protocol placeholder is safe. Sizes are generous
/// "unknown but sane" defaults so the model's own cap doesn't truncate the
/// user's request; the actual limits come from `ChatOptions`.
fn synthesize_custom_model(model_id: &str, api: model::Api) -> model::Model {
    let provider = match api {
        model::Api::AnthropicMessages => model::types::Provider::Anthropic,
        _ => model::types::Provider::Openrouter,
    };
    model::Model {
        id: model_id.to_string(),
        name: model_id.to_string(),
        api,
        provider,
        base_url: String::new(),
        reasoning: false,
        input: vec![model::InputType::Text],
        cost: model::Cost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
        headers: None,
        // None lets hand-ai auto-detect OpenAI-compat quirks from base_url.
        compat: None,
        thinking_level_map: None,
    }
}

#[allow(clippy::field_reassign_with_default)]
// StreamOptions and SimpleStreamOptions are #[non_exhaustive] in hand_ai_model
// (since #32 / commit 7994163). FRU (`..Default::default()`) is illegal from
// outside the defining crate, so we mutate-default.
pub(crate) fn build_stream_options(options: &ChatOptions, api_key: &str) -> SimpleStreamOptions {
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

/// Harvest tool calls from a terminal assistant message's content blocks.
/// Returns `None` if no `ToolCall` blocks were present.
fn tool_calls_from_assistant_message(message: &model::AssistantMessage) -> Option<Vec<ChatToolCall>> {
    let calls: Vec<ChatToolCall> = message
        .content
        .iter()
        .filter_map(|block| match block {
            model::AssistantContentBlock::ToolCall(tc) => Some(ChatToolCall {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments: serde_json::to_string(&tc.arguments)
                    .unwrap_or_else(|_| "{}".to_string()),
            }),
            _ => None,
        })
        .collect();
    if calls.is_empty() {
        None
    } else {
        Some(calls)
    }
}

/// Translate one hand-ai event to one HandBox chunk, if applicable.
///
/// `*_Start` / `*_End` boundary variants emit nothing (the matching `_Delta`
/// events carry the payload). `Error` is translated to `Err` so the consumer's
/// stream terminates with the failure rather than receiving a synthetic chunk.
/// On the terminal `Done` event we also harvest any `ToolCall` blocks from the
/// assistant message's content into `ChatChunk::tool_calls`.
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
            tool_calls: tool_calls_from_assistant_message(message),
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
        supported_methods: Some(supported_methods_for_api(m.api)),
        model_created_at: None,
        enabled: true,
        favorite: false,
        created_at: now,
        updated_at: now,
    }
}

/// Map a hand-ai `Api` enum value to the short chat-method tags that
/// HandBox's UI uses to pick a parameter set. Pre-dissolve, only the
/// OpenRouter adapter populated this field (with `"completions"`); the
/// other legacy fetchers left it `None`, so existing DB rows for OpenAI
/// / Anthropic providers may also be empty in the same way. After M2-T4
/// rewired through hand-ai's catalog every freshly-synced provider
/// must populate it explicitly or `is_method_supported` short-circuits
/// to false and `get_provider_models` silently filters every model out
/// at the IPC boundary (the bug that hid all 3 Cerebras models from
/// the picker until this fix).
fn supported_methods_for_api(api: model::Api) -> Vec<String> {
    use model::Api;
    match api {
        // OpenAI-Completions wire family.
        Api::OpenAICompletions | Api::MistralConversations => vec!["completions".to_string()],
        // OpenAI-Responses wire family.
        Api::OpenAIResponses
        | Api::AzureOpenAiResponses
        | Api::OpenAICodexResponses => vec!["responses".to_string()],
        // Native Anthropic / Bedrock-Converse. The UI parameter set is
        // identical to OpenAI Completions (temperature / top_p / max_tokens /
        // streaming), and the actual chat wire dispatch is owned by hand-ai's
        // Client based on `Model.api`, not by this string. Tagging as
        // `"completions"` lets `ChatMethod::Completions` pick up its base
        // parameter rendering for these providers.
        Api::AnthropicMessages | Api::BedrockConverseStream => vec!["completions".to_string()],
        // Google native Generate-Content family.
        Api::GoogleGenerativeAi | Api::GoogleGeminiCli | Api::GoogleVertex => {
            vec!["google_generate_content".to_string()]
        }
        // In-memory test harness — no UI surface.
        Api::Faux => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hand_ai_model::UserContent;

    #[test]
    fn supported_methods_for_api_covers_every_variant() {
        // Pin the per-Api mapping so a new hand-ai Api variant forces an
        // explicit decision here (the exhaustive match in
        // `supported_methods_for_api` already guarantees compile-time
        // failure; this test pins runtime semantics for the existing set).
        use hand_ai_model::Api;

        assert_eq!(
            supported_methods_for_api(Api::OpenAICompletions),
            vec!["completions".to_string()],
            "OpenAI-Completions wire → ChatMethod::Completions parameter set"
        );
        assert_eq!(
            supported_methods_for_api(Api::OpenAIResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::AzureOpenAiResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::OpenAICodexResponses),
            vec!["responses".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::AnthropicMessages),
            vec!["completions".to_string()],
            "Anthropic wire uses Completions parameter set for UI"
        );
        assert_eq!(
            supported_methods_for_api(Api::BedrockConverseStream),
            vec!["completions".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleGenerativeAi),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleGeminiCli),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::GoogleVertex),
            vec!["google_generate_content".to_string()],
        );
        assert_eq!(
            supported_methods_for_api(Api::MistralConversations),
            vec!["completions".to_string()],
        );
        assert!(
            supported_methods_for_api(Api::Faux).is_empty(),
            "Faux is the in-memory test harness — no UI surface"
        );
    }

    // ---- ChatMessage fixture helpers -----------------------------------

    fn user_msg(text: &str) -> ChatMessage {
        ChatMessage {
            id: "msg".to_string(),
            role: LlmMessageRole::User,
            content: text.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachment_ids: vec![],
        }
    }

    fn system_msg(text: &str) -> ChatMessage {
        ChatMessage {
            id: "msg".to_string(),
            role: LlmMessageRole::System,
            content: text.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachment_ids: vec![],
        }
    }

    fn assistant_msg(text: &str) -> ChatMessage {
        ChatMessage {
            id: "msg".to_string(),
            role: LlmMessageRole::Assistant,
            content: text.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachment_ids: vec![],
        }
    }

    fn tool_msg(text: &str, tool_call_id: &str) -> ChatMessage {
        ChatMessage {
            id: "msg".to_string(),
            role: LlmMessageRole::Tool,
            content: text.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            attachment_ids: vec![],
        }
    }

    fn tool_call(id: &str, name: &str, arguments: &str) -> ChatToolCall {
        ChatToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments: arguments.to_string(),
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

    // ---- messages_to_context ------------------------------------------

    #[test]
    fn builds_context_from_user_message() {
        let messages = vec![user_msg("hello")];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect("translation");
        assert!(ctx.system_prompt.is_none());
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn extracts_system_prompt_from_leading_system_message() {
        let messages = vec![system_msg("be helpful"), user_msg("hi")];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect("translation");
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
        assert!(chunk.tool_calls.is_none());
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
        // No ToolCall blocks in the message → tool_calls stays None.
        assert!(chunk.tool_calls.is_none());
    }

    #[test]
    fn assistant_history_errors_when_model_not_in_catalog() {
        // "openai" is a catalog provider but the model doesn't exist, and
        // "openai" is not a custom type → no synthesis fallback → clear error.
        let messages = vec![assistant_msg("prior reply")];
        let err = messages_to_context(&messages, "openai", "no-such-model-9999", &ChatOptions::default())
            .expect_err("unknown model must surface clearly");
        let m = format!("{}", err);
        assert!(m.contains("not registered under provider"), "msg: {m}");
        assert!(m.contains("no-such-model-9999"), "msg: {m}");
    }

    #[test]
    fn every_listed_openrouter_model_is_resolvable() {
        // Regression: the model picker is built from `get_models()`, which listed
        // OpenRouter ":free" variants (e.g. `deepseek/deepseek-v4-flash:free`)
        // that `get_model()` couldn't resolve by id — so creating a session with
        // one and running it errored ("not registered under provider 'openrouter'").
        // The catalog-list fallback closes that gap: anything the picker offers
        // must now resolve.
        let listed = hand_ai_model::get_models("openrouter");
        assert!(!listed.is_empty(), "expected a non-empty OpenRouter catalog");
        for m in &listed {
            assert!(
                resolve_model_template("openrouter", &m.id).is_ok(),
                "picker lists openrouter/{} but resolve_model_template rejects it",
                m.id
            );
        }
    }

    #[test]
    fn custom_api_mapping_covers_known_types() {
        assert_eq!(
            custom_api_for_provider_type("openai-compatible"),
            Some(hand_ai_model::Api::OpenAICompletions)
        );
        assert_eq!(
            custom_api_for_provider_type("anthropic-compatible"),
            Some(hand_ai_model::Api::AnthropicMessages)
        );
        assert_eq!(custom_api_for_provider_type("openai"), None);
        assert_eq!(custom_api_for_provider_type("groq"), None);
    }

    #[test]
    fn custom_provider_supported_methods_maps_to_completions() {
        assert_eq!(
            custom_provider_supported_methods("openai-compatible"),
            Some(vec!["completions".to_string()])
        );
        assert_eq!(
            custom_provider_supported_methods("anthropic-compatible"),
            Some(vec!["completions".to_string()])
        );
        assert_eq!(custom_provider_supported_methods("openai"), None);
    }

    #[test]
    fn synthesize_custom_model_builds_streamable_template() {
        // openai-compatible → OpenAICompletions template, dispatch-ready.
        let m = synthesize_custom_model("my-local-llm", hand_ai_model::Api::OpenAICompletions);
        assert_eq!(m.id, "my-local-llm");
        assert_eq!(m.api, hand_ai_model::Api::OpenAICompletions);
        // provider is a same-protocol placeholder, never GitHubCopilot (which
        // would trip the special-case in the openai-completions provider).
        assert_ne!(m.provider, hand_ai_model::types::Provider::GitHubCopilot);
        assert!(m.max_tokens > 0, "non-zero cap so requests aren't truncated");
        assert!(m.compat.is_none(), "compat auto-detected from base_url");

        // anthropic-compatible → AnthropicMessages + Anthropic placeholder.
        let a = synthesize_custom_model("claude-proxy", hand_ai_model::Api::AnthropicMessages);
        assert_eq!(a.api, hand_ai_model::Api::AnthropicMessages);
        assert_eq!(a.provider, hand_ai_model::types::Provider::Anthropic);
    }

    #[test]
    fn custom_provider_assistant_history_synthesizes_instead_of_erroring() {
        // A custom provider with prior assistant turns must NOT error — it
        // synthesizes the assistant_meta from the custom type's protocol.
        let messages = vec![assistant_msg("prior reply")];
        let ctx = messages_to_context(
            &messages,
            "openai-compatible",
            "my-local-llm",
            &ChatOptions::default(),
        )
        .expect("custom provider should synthesize, not error");
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn tool_result_pulls_tool_name_from_prior_assistant_call() {
        let mut assistant = assistant_msg("");
        assistant.tool_calls = Some(vec![tool_call("call_99", "search_web", "{}")]);
        let tool_result = tool_msg("result text", "call_99");

        let messages = vec![assistant, tool_result];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect("translation");
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
        let orphan = tool_msg("orphan", "ghost");
        let messages = vec![orphan];
        let err = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect_err("orphan tool result should error");
        assert!(format!("{}", err).contains("unknown tool_call_id"));
    }

    #[test]
    fn assistant_history_preserves_reasoning_and_tool_calls() {
        let mut assistant = assistant_msg("calling tool");
        assistant.reasoning = Some("considering options".to_string());
        assistant.tool_calls = Some(vec![tool_call("call_1", "get_weather", r#"{"city":"sf"}"#)]);

        let messages = vec![assistant];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect("translation");
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
            system_msg("first"),
            user_msg("hi"),
            system_msg("second"),
        ];
        let ctx = messages_to_context(&messages, "openai", "gpt-4o", &ChatOptions::default())
            .expect("translation");
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

    // ---- New in M2-T2a ------------------------------------------------

    #[test]
    fn terminal_chunk_aggregates_tool_calls_from_done_event() {
        let mut msg = partial_assistant();
        msg.content = vec![
            model::AssistantContentBlock::ToolCall(model::ToolCall::new(
                "call_a".to_string(),
                "search_web".to_string(),
                serde_json::json!({"q": "x"}),
            )),
            model::AssistantContentBlock::ToolCall(model::ToolCall::new(
                "call_b".to_string(),
                "fetch_url".to_string(),
                serde_json::json!({"u": "y"}),
            )),
        ];
        let event = AssistantMessageEvent::Done {
            reason: StopReason::ToolUse,
            message: msg,
        };
        let chunk = event_to_chunk_result(&event)
            .expect("event maps")
            .expect("ok variant");
        let calls = chunk.tool_calls.expect("tool_calls populated");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_a");
        assert_eq!(calls[0].name, "search_web");
        assert_eq!(calls[1].id, "call_b");
        assert_eq!(calls[1].name, "fetch_url");

        let args_a: serde_json::Value = serde_json::from_str(&calls[0].arguments)
            .expect("call_a arguments round-trip");
        assert_eq!(args_a, serde_json::json!({"q": "x"}));
        let args_b: serde_json::Value = serde_json::from_str(&calls[1].arguments)
            .expect("call_b arguments round-trip");
        assert_eq!(args_b, serde_json::json!({"u": "y"}));

        assert_eq!(chunk.finish_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn user_attachment_translates_to_image_block() {
        let msg = ChatMessage {
            id: "m1".to_string(),
            role: LlmMessageRole::User,
            content: "what's this?".to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachment_ids: vec!["m1-att-0".to_string()],
        };
        let mut hydrated: HashMap<String, Vec<HydratedAttachment>> = HashMap::new();
        hydrated.insert(
            "m1".to_string(),
            vec![HydratedAttachment {
                name: "shot.png".to_string(),
                mime_type: "image/png".to_string(),
                data: vec![0x89, 0x50, 0x4e, 0x47],
            }],
        );
        let options = ChatOptions {
            hydrated_attachments: hydrated,
            ..Default::default()
        };

        let ctx = messages_to_context(&[msg], "openai", "gpt-4o", &options).expect("translation");
        assert_eq!(ctx.messages.len(), 1);
        match &ctx.messages[0] {
            Message::User(um) => match &um.content {
                UserContent::Blocks(blocks) => {
                    assert_eq!(blocks.len(), 2);
                    match &blocks[0] {
                        UserContentBlock::Image(img) => {
                            assert_eq!(img.mime_type, "image/png");
                            // base64 of PNG magic bytes (0x89 0x50 0x4e 0x47).
                            assert_eq!(img.data, "iVBORw==");
                        }
                        other => panic!("expected image first, got {:?}", other),
                    }
                    match &blocks[1] {
                        UserContentBlock::Text(t) => assert_eq!(t.text, "what's this?"),
                        other => panic!("expected text second, got {:?}", other),
                    }
                }
                _ => panic!("expected blocks variant for attachment message"),
            },
            other => panic!("expected user message, got {:?}", other),
        }
    }
}
