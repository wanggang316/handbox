// hand-ai adapter — routes ChatClient calls through hand-ai/crates/model.
//
// Behind the `hand-ai` feature flag. Dark by default. See
// docs/exec-plans/hand-ai-integration.md for the full translation table
// and milestones; this file implements M1 (compile-clean skeleton).
//
// What this adapter delivers in M1:
//   • Compiles when `--features hand-ai` is set.
//   • `ChatClient::api_type()` returns "hand-ai".
//   • `chat()` and `chat_stream()` translate `LlmRequest` → `model::Context`
//     and back, but `unimplemented!()` on paths that depend on translation
//     details not yet finalized (image attachments, tool-result encoding,
//     reasoning/thinking config, generated images).
//
// What lands in later milestones:
//   M2 — wire one provider (openai) end-to-end through `complete_simple`
//        / `stream_simple` with real network calls; pass live HandBox chat.
//   M3 — parity for anthropic / google / openrouter and delete the
//        per-provider adapters.
//   M4 — drive provider catalog from hand-ai's `get_providers()` /
//        `ProviderCapabilities` (hand-ai issue #31).

use async_trait::async_trait;
use futures::{Stream, StreamExt};

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmMessage, LlmMessageRole, LlmProvider,
    LlmRequest, LlmRequestTool, LlmResponse, LlmUsage,
};

use hand_ai_model::{
    self as model, AssistantMessageEvent, Client, Context, Message, SimpleStreamOptions,
    StopReason, StreamOptions, Tool, Usage, UserMessage,
};

/// `ChatClient` that delegates to `hand_ai_model::stream_simple` /
/// `complete_simple`.
///
/// One instance is bound to a single hand-ai provider id (e.g. "openai",
/// "anthropic"). HandBox's `create_chat_client(LlmApiType)` will produce one
/// of these per supported provider once parity is verified.
pub struct HandAiChatClient {
    /// Hand-ai provider id this adapter targets. Matches `model::Provider`
    /// values exposed by `hand_ai_model::get_providers()`.
    provider_id: &'static str,
    /// hand-ai `Client` (holds the `Arc<ApiProviderRegistry>`). Cheap to clone.
    client: Client,
}

impl HandAiChatClient {
    pub fn new(provider_id: &'static str) -> Self {
        Self {
            provider_id,
            client: Client::new(),
        }
    }

    /// hand-ai provider id this adapter targets.
    pub fn provider_id(&self) -> &'static str {
        self.provider_id
    }
}

#[async_trait]
impl ChatClient for HandAiChatClient {
    async fn chat(
        &self,
        _provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        // M1 stub: translation compiles, but we don't actually invoke the
        // network yet. Live call lands in M2.
        let _context = llm_request_to_context(&request)?;
        let _tools = request.tools.as_ref().map(translate_tools).transpose()?;
        Err(LlmClientError::validation(
            "hand-ai adapter chat() not yet wired; lands in M2",
        ))
    }

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        let context = llm_request_to_context(&request)?;
        let model = resolve_model(self.provider_id, &request.model, &provider.base_url)?;
        let options = build_stream_options(&request, &provider.api_key);

        let event_stream = hand_ai_model::stream_simple(
            &self.client.registry,
            &model,
            context,
            Some(options),
        )
        .map_err(client_err_to_handbox)?;

        let model_id = request.model.clone();
        let chunk_stream = event_stream.filter_map(move |event| {
            let model_id = model_id.clone();
            async move { event_to_chunk_result(&event, &model_id) }
        });

        Ok(Box::new(Box::pin(chunk_stream)))
    }

    fn api_type(&self) -> &'static str {
        "hand-ai"
    }
}

// ---------------------------------------------------------------------------
// Translation: LlmRequest → model::Context
// ---------------------------------------------------------------------------

/// Convert HandBox's `LlmRequest` into hand-ai's `Context`.
///
/// HandBox encodes the system prompt as a `LlmMessageRole::System` entry in
/// `messages`; hand-ai expects it on `Context::system_prompt`. We strip the
/// first system message out into `system_prompt` and concatenate any later
/// system messages into it with a blank line between them (rare in practice
/// but happens with multi-stage prompts).
pub(crate) fn llm_request_to_context(request: &LlmRequest) -> Result<Context, LlmClientError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut messages: Vec<Message> = Vec::with_capacity(request.messages.len());

    for msg in &request.messages {
        match msg.role {
            LlmMessageRole::System => {
                if !msg.content.is_empty() {
                    system_chunks.push(msg.content.clone());
                }
            }
            LlmMessageRole::User => {
                messages.push(Message::User(llm_user_message(msg)?));
            }
            LlmMessageRole::Assistant => {
                // M1: reconstructing a prior assistant turn requires api/
                // provider/model/usage/stop_reason metadata that HandBox does
                // not carry on LlmMessage. Multi-turn replay lands in M2 once
                // we decide whether to (a) store this metadata in HandBox or
                // (b) ask hand-ai for a lightweight history-only constructor.
                return Err(LlmClientError::validation(
                    "hand-ai adapter: prior assistant messages in history not yet supported (M2)",
                ));
            }
            LlmMessageRole::Tool => {
                // M1: ToolResultMessage requires tool_name, which HandBox's
                // LlmMessage(role=Tool) doesn't carry. Need to thread it
                // through from the upstream tool-call record. Lands in M2.
                return Err(LlmClientError::validation(
                    "hand-ai adapter: tool result messages not yet supported (M2)",
                ));
            }
        }
    }

    let system_prompt = if system_chunks.is_empty() {
        None
    } else {
        Some(system_chunks.join("\n\n"))
    };

    // Tools translated separately by callers that need them on the registry
    // call site (hand-ai's `stream_simple` consumes them via Context).
    let tools = request.tools.as_ref().map(translate_tools).transpose()?;

    Ok(Context {
        system_prompt,
        messages,
        tools,
    })
}

fn llm_user_message(msg: &LlmMessage) -> Result<UserMessage, LlmClientError> {
    // M1: text-only path. Image / file attachments lands in M2.
    if msg.attachments.is_some() {
        return Err(LlmClientError::validation(
            "hand-ai adapter: user message attachments not yet supported (M2)",
        ));
    }
    Ok(UserMessage::new_text(msg.content.clone()))
}

fn translate_tools(tools: &Vec<LlmRequestTool>) -> Result<Vec<Tool>, LlmClientError> {
    tools
        .iter()
        .map(|t| {
            Ok(Tool::new(
                t.function.name.clone(),
                t.function.description.clone(),
                t.function.parameters.clone(),
            ))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Model lookup + options building
// ---------------------------------------------------------------------------

/// Look up the hand-ai `Model` template and override `base_url` from the
/// caller's `LlmProvider`.
fn resolve_model(
    provider_id: &str,
    model_id: &str,
    base_url: &str,
) -> Result<model::Model, LlmClientError> {
    let mut m = hand_ai_model::get_model(provider_id, model_id).ok_or_else(|| {
        LlmClientError::validation(format!(
            "hand-ai: model '{}' not registered under provider '{}'",
            model_id, provider_id
        ))
    })?;
    if !base_url.is_empty() {
        m.base_url = base_url.to_string();
    }
    Ok(m)
}

fn build_stream_options(request: &LlmRequest, api_key: &str) -> SimpleStreamOptions {
    let mut base = StreamOptions::default();
    base.api_key = Some(api_key.to_string());
    base.temperature = request.temperature;
    base.max_tokens = request.max_tokens.and_then(|v| u32::try_from(v).ok());
    SimpleStreamOptions {
        base,
        // reasoning/thinking_budgets: HandBox passes these through
        // `request.reasoning_effort` / `request.thinking` today; mapping
        // lands in M3 alongside the other provider parity work.
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Translation: AssistantMessageEvent stream → LlmChunkResponse stream
// ---------------------------------------------------------------------------

fn client_err_to_handbox(err: hand_ai_model::ClientError) -> LlmClientError {
    LlmClientError::validation(format!("hand-ai client error: {}", err))
}

fn chunk_envelope(
    model_id: &str,
    delta: Option<LlmDeltaMessage>,
    finish_reason: Option<String>,
    usage: Option<LlmUsage>,
) -> LlmChunkResponse {
    LlmChunkResponse {
        id: String::new(),
        object: "chat.completion.chunk".to_string(),
        model: model_id.to_string(),
        choices: vec![LlmChunkChoice {
            index: 0,
            delta,
            finish_reason,
            generated_images: None,
        }],
        usage,
    }
}

fn usage_to_llm(usage: &Usage) -> Option<LlmUsage> {
    if usage.input == 0 && usage.output == 0 && usage.total_tokens == 0 {
        return None;
    }
    Some(LlmUsage {
        prompt_tokens: i32::try_from(usage.input).unwrap_or(i32::MAX),
        completion_tokens: i32::try_from(usage.output).unwrap_or(i32::MAX),
        total_tokens: i32::try_from(usage.total_tokens).unwrap_or(i32::MAX),
    })
}

/// Map one hand-ai event to one HandBox chunk, if applicable.
///
/// `*_Start` / `*_End` variants emit nothing (the matching `_Delta` events
/// carry the payload, and HandBox doesn't need explicit boundary markers).
/// `Error` is translated to `Err` so the consumer's stream terminates with
/// the failure rather than receiving a synthetic content chunk.
fn event_to_chunk_result(
    event: &AssistantMessageEvent,
    model_id: &str,
) -> Option<Result<LlmChunkResponse, LlmClientError>> {
    match event {
        AssistantMessageEvent::TextDelta { delta, .. } => Some(Ok(chunk_envelope(
            model_id,
            Some(LlmDeltaMessage {
                role: None,
                content: Some(delta.clone()),
                reasoning: None,
                tool_calls: None,
            }),
            None,
            None,
        ))),
        AssistantMessageEvent::ThinkingDelta { delta, .. } => Some(Ok(chunk_envelope(
            model_id,
            Some(LlmDeltaMessage {
                role: None,
                content: None,
                reasoning: Some(delta.clone()),
                tool_calls: None,
            }),
            None,
            None,
        ))),
        AssistantMessageEvent::Done { reason, message } => Some(Ok(chunk_envelope(
            model_id,
            None,
            Some(stop_reason_to_finish_reason(reason).to_string()),
            usage_to_llm(&message.usage),
        ))),
        AssistantMessageEvent::Error { error, .. } => {
            let msg = error
                .error_message
                .clone()
                .unwrap_or_else(|| "hand-ai stream returned Error event".to_string());
            Some(Err(LlmClientError::validation(msg)))
        }
        // Start / TextStart / TextEnd / ThinkingStart / ThinkingEnd produce
        // no downstream chunk. ToolCall* variants land in M3 alongside tool
        // call parity (HandBox's LlmDeltaToolCall has its own aggregation
        // semantics that need a dedicated translation block).
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hand_ai_model::UserContent;

    fn user_msg(text: &str) -> LlmMessage {
        LlmMessage {
            role: LlmMessageRole::User,
            content: text.into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
        }
    }

    fn system_msg(text: &str) -> LlmMessage {
        LlmMessage {
            role: LlmMessageRole::System,
            content: text.into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
        }
    }

    fn bare_request(messages: Vec<LlmMessage>) -> LlmRequest {
        LlmRequest {
            model: "gpt-4o".into(),
            messages,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            stream: None,
            reasoning: None,
            reasoning_effort: None,
            thinking: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        }
    }

    #[test]
    fn text_only_user_messages_translate_to_user_blocks() {
        let req = bare_request(vec![user_msg("hello"), user_msg("world")]);
        let ctx = llm_request_to_context(&req).expect("translation");
        assert!(ctx.system_prompt.is_none());
        assert_eq!(ctx.messages.len(), 2);
        match &ctx.messages[0] {
            Message::User(um) => match &um.content {
                UserContent::Text(t) => assert_eq!(t, "hello"),
                UserContent::Blocks(_) => panic!("expected text variant"),
            },
            other => panic!("expected user, got {:?}", other),
        }
    }

    #[test]
    fn system_message_extracted_into_system_prompt() {
        let req = bare_request(vec![system_msg("you are helpful"), user_msg("hi")]);
        let ctx = llm_request_to_context(&req).expect("translation");
        assert_eq!(ctx.system_prompt.as_deref(), Some("you are helpful"));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn multiple_system_messages_concatenated_with_blank_line() {
        let req = bare_request(vec![
            system_msg("first"),
            user_msg("hi"),
            system_msg("second"),
        ]);
        let ctx = llm_request_to_context(&req).expect("translation");
        assert_eq!(ctx.system_prompt.as_deref(), Some("first\n\nsecond"));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn empty_system_message_dropped() {
        let req = bare_request(vec![system_msg(""), user_msg("hi")]);
        let ctx = llm_request_to_context(&req).expect("translation");
        assert!(ctx.system_prompt.is_none());
    }

    #[test]
    fn assistant_history_returns_validation_error_until_m2() {
        let req = bare_request(vec![LlmMessage {
            role: LlmMessageRole::Assistant,
            content: "prior reply".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
        }]);
        let err = llm_request_to_context(&req).expect_err("M1 should reject assistant history");
        assert!(format!("{}", err).contains("M2"));
    }

    // ---- event_to_chunk_result -----------------------------------------

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
    fn text_delta_becomes_content_chunk() {
        let event = AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "hi".into(),
            partial: partial_assistant(),
        };
        let chunk = event_to_chunk_result(&event, "gpt-4o")
            .expect("event maps")
            .expect("ok variant");
        assert_eq!(chunk.choices[0].delta.as_ref().unwrap().content.as_deref(), Some("hi"));
        assert!(chunk.choices[0].finish_reason.is_none());
    }

    #[test]
    fn thinking_delta_becomes_reasoning_chunk() {
        let event = AssistantMessageEvent::ThinkingDelta {
            content_index: 0,
            delta: "let me think".into(),
            partial: partial_assistant(),
        };
        let chunk = event_to_chunk_result(&event, "gpt-4o")
            .expect("event maps")
            .expect("ok variant");
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().reasoning.as_deref(),
            Some("let me think")
        );
        assert!(chunk.choices[0].delta.as_ref().unwrap().content.is_none());
    }

    #[test]
    fn done_event_produces_terminal_chunk_with_finish_reason() {
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
        let chunk = event_to_chunk_result(&event, "gpt-4o")
            .expect("event maps")
            .expect("ok variant");
        assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
        let usage = chunk.usage.expect("usage forwarded");
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 10);
        assert_eq!(usage.total_tokens, 15);
    }

    #[test]
    fn error_event_translates_to_err() {
        let mut msg = partial_assistant();
        msg.error_message = Some("upstream blew up".into());
        let event = AssistantMessageEvent::Error {
            reason: StopReason::Error,
            error: msg,
        };
        let result = event_to_chunk_result(&event, "gpt-4o").expect("event maps");
        let err = result.expect_err("error event must surface as Err");
        assert!(format!("{}", err).contains("upstream blew up"));
    }

    #[test]
    fn boundary_events_emit_nothing() {
        let event = AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: partial_assistant(),
        };
        assert!(event_to_chunk_result(&event, "gpt-4o").is_none());
    }
}

/// Map hand-ai's terminal `StopReason` to HandBox's `finish_reason` string.
#[allow(dead_code)] // M2
pub(crate) fn stop_reason_to_finish_reason(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Stop => "stop",
        StopReason::Length => "length",
        StopReason::ToolUse => "tool_calls",
        StopReason::Aborted => "stop",
        // `Error` is propagated as `LlmClientError` upstream of this fn; if it
        // somehow reaches here, surface it as a plain stop rather than panic.
        StopReason::Error => "stop",
    }
}
