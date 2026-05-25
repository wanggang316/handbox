// hand-ai adapter — routes ChatClient calls through hand-ai/crates/model.
//
// Behind the `hand-ai` feature flag. See docs/exec-plans/hand-ai-integration.md
// for the full translation table and milestones.
//
// What this adapter delivers today:
//   • Compiles when `--features hand-ai` is set.
//   • `ChatClient::api_type()` returns "hand-ai".
//   • `chat_stream()` invokes hand_ai_model::stream_simple end-to-end:
//     LlmRequest → Context (with prior assistant turns reconstructed
//     from the current request's model template, tool_name pulled from
//     preceding tool_calls), per-request api_key + base_url override
//     via StreamOptions / Model.base_url, and an event aggregator that
//     maps TextDelta/ThinkingDelta/Done/Error back to LlmChunkResponse.
//   • Image attachments translate to UserContentBlock::Image.
//
// What still validation-errors out:
//   • Non-image attachments (PDF, etc.) — needs hand-ai content variants.
//   • Models not in hand-ai's static catalog — adapter requires a Model
//     template lookup; synthetic Model construction lands in M3.
//   • Non-stream `chat()` path — wired in M3 alongside legacy adapter
//     deletion.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use futures::{Stream, StreamExt};

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmMessage, LlmMessageRole, LlmProvider,
    LlmRequest, LlmRequestTool, LlmResponse, LlmUsage,
};

use hand_ai_model::{
    self as model, AssistantMessageEvent, Client, Context, ImageContent, Message,
    SimpleStreamOptions, StopReason, StreamOptions, TextContent, Tool, Usage, UserContentBlock,
    UserMessage,
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
        let _context = llm_request_to_context(&request, self.provider_id)?;
        let _tools = request.tools.as_ref().map(translate_tools).transpose()?;
        Err(LlmClientError::validation(
            "hand-ai adapter chat() not yet wired; lands in M3",
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
        let context = llm_request_to_context(&request, self.provider_id)?;
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
pub(crate) fn llm_request_to_context(
    request: &LlmRequest,
    provider_id: &str,
) -> Result<Context, LlmClientError> {
    let mut system_chunks: Vec<String> = Vec::new();
    let mut messages: Vec<Message> = Vec::with_capacity(request.messages.len());
    // Threaded so role=Tool entries can recover the tool name from the
    // preceding role=Assistant turn that declared the call. HandBox's
    // LlmMessage(role=Tool) only carries `tool_call_id`; hand-ai requires
    // `tool_name` on ToolResultMessage.
    let mut tool_name_by_call_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // For prior assistant turns, hand-ai's AssistantMessage requires
    // api/provider/model metadata HandBox doesn't store per-message. We
    // best-effort reconstruct from the *current* request's model: even if
    // the past turn ran on a different model, the historical record is
    // about *content*, not provenance — and hand-ai's transcript handling
    // doesn't validate that prior turns match the current model. If the
    // current model id isn't in hand-ai's catalog we surface a clear
    // error rather than fabricating Api::OpenAICompletions / Provider::OpenAI.
    let assistant_meta = if request
        .messages
        .iter()
        .any(|m| m.role == LlmMessageRole::Assistant)
    {
        let template = hand_ai_model::get_model(provider_id, &request.model).ok_or_else(|| {
            LlmClientError::validation(format!(
                "hand-ai: cannot reconstruct prior assistant turn — model '{}' under provider '{}' not in catalog",
                request.model, provider_id
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
                if let Some(calls) = msg.tool_calls.as_ref() {
                    for c in calls {
                        tool_name_by_call_id.insert(c.id.clone(), c.function.name.clone());
                    }
                }
                let meta = assistant_meta
                    .as_ref()
                    .expect("computed when any assistant entry exists");
                messages.push(Message::Assistant(llm_assistant_message(msg, meta)?));
            }
            LlmMessageRole::Tool => {
                messages.push(Message::ToolResult(llm_tool_result_message(
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

    // Tools translated separately by callers that need them on the registry
    // call site (hand-ai's `stream_simple` consumes them via Context).
    let tools = request.tools.as_ref().map(translate_tools).transpose()?;

    Ok(Context {
        system_prompt,
        messages,
        tools,
    })
}

/// api / provider / model triple recovered from the current request's
/// model, used to fill required fields on reconstructed prior assistant
/// turns. Cached once per request so multi-turn histories don't pay
/// the catalog lookup per message.
struct AssistantMeta {
    api: model::Api,
    provider: hand_ai_model::types::Provider,
    model: String,
}

fn llm_user_message(msg: &LlmMessage) -> Result<UserMessage, LlmClientError> {
    let Some(attachments) = msg.attachments.as_ref().filter(|a| !a.is_empty()) else {
        return Ok(UserMessage::new_text(msg.content.clone()));
    };

    // At least one attachment — build a block sequence: image(s) first, then
    // the text body if non-empty. Mirrors what HandBox does for openai
    // completions (image_url precedes text in the request content array).
    let mut blocks: Vec<UserContentBlock> = Vec::with_capacity(attachments.len() + 1);
    for att in attachments {
        if !att.mime_type.starts_with("image/") {
            // M3 will add file / pdf passthrough once hand-ai has a matching
            // UserContentBlock variant. For now, refuse explicitly so we
            // never silently drop a user's attached file.
            return Err(LlmClientError::validation(format!(
                "hand-ai adapter: non-image attachment '{}' (mime: {}) not yet supported (M3)",
                att.name, att.mime_type
            )));
        }
        let data_b64 = BASE64_STANDARD.encode(&att.data);
        blocks.push(UserContentBlock::Image(ImageContent::new(
            data_b64,
            att.mime_type.clone(),
        )));
    }
    if !msg.content.is_empty() {
        blocks.push(UserContentBlock::Text(TextContent::new(msg.content.clone())));
    }
    Ok(UserMessage::new_blocks(blocks))
}

fn llm_assistant_message(
    msg: &LlmMessage,
    meta: &AssistantMeta,
) -> Result<model::AssistantMessage, LlmClientError> {
    let mut content: Vec<model::AssistantContentBlock> = Vec::new();

    // Reasoning → ThinkingContent. Some providers carry reasoning as a
    // separate sidecar field on assistant turns; preserve it so the next
    // turn sees the same context the user did.
    if let Some(reasoning) = msg.reasoning.as_ref().filter(|s| !s.is_empty()) {
        content.push(model::AssistantContentBlock::Thinking(
            model::ThinkingContent::new(reasoning.clone()),
        ));
    }

    if !msg.content.is_empty() {
        content.push(model::AssistantContentBlock::Text(
            model::TextContent::new(msg.content.clone()),
        ));
    }

    if let Some(calls) = msg.tool_calls.as_ref() {
        for call in calls {
            // HandBox stores arguments as a JSON-encoded string;
            // hand-ai's ToolCall.arguments is a Value.
            let args: serde_json::Value = if call.function.arguments.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_json::from_str(&call.function.arguments).map_err(|e| {
                    LlmClientError::validation(format!(
                        "hand-ai adapter: tool_call arguments not valid JSON for call '{}': {}",
                        call.id, e
                    ))
                })?
            };
            content.push(model::AssistantContentBlock::ToolCall(model::ToolCall::new(
                call.id.clone(),
                call.function.name.clone(),
                args,
            )));
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

fn llm_tool_result_message(
    msg: &LlmMessage,
    tool_name_by_call_id: &std::collections::HashMap<String, String>,
) -> Result<model::ToolResultMessage, LlmClientError> {
    let tool_call_id = msg.tool_call_id.as_ref().ok_or_else(|| {
        LlmClientError::validation("hand-ai adapter: LlmMessage role=Tool requires tool_call_id")
    })?;
    let tool_name = tool_name_by_call_id.get(tool_call_id).ok_or_else(|| {
        LlmClientError::validation(format!(
            "hand-ai adapter: tool result for unknown tool_call_id '{}' (no preceding assistant tool_call with this id)",
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
    // Both Stream/SimpleStreamOptions are #[non_exhaustive] in hand-ai
    // since #32 (commit 7994163) — must use mutate-default, not FRU.
    // reasoning/thinking_budgets: HandBox passes these through
    // `request.reasoning_effort` / `request.thinking` today; mapping
    // lands in M3 alongside the other provider parity work.
    let mut opts = SimpleStreamOptions::default();
    opts.base = base;
    opts
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
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
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
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
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
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        assert_eq!(ctx.system_prompt.as_deref(), Some("first\n\nsecond"));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn empty_system_message_dropped() {
        let req = bare_request(vec![system_msg(""), user_msg("hi")]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        assert!(ctx.system_prompt.is_none());
    }

    #[test]
    fn assistant_history_translates_when_model_in_catalog() {
        let req = bare_request(vec![
            user_msg("hi"),
            LlmMessage {
                role: LlmMessageRole::Assistant,
                content: "prior reply".into(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
                attachments: None,
            },
            user_msg("follow-up"),
        ]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        assert_eq!(ctx.messages.len(), 3);
        match &ctx.messages[1] {
            Message::Assistant(am) => {
                assert_eq!(am.model, "gpt-4o");
                match &am.content[..] {
                    [model::AssistantContentBlock::Text(t)] => {
                        assert_eq!(t.text, "prior reply");
                    }
                    other => panic!("expected single text block, got {:?}", other),
                }
            }
            other => panic!("expected assistant, got {:?}", other),
        }
    }

    #[test]
    fn assistant_history_errors_when_model_not_in_catalog() {
        let mut req = bare_request(vec![LlmMessage {
            role: LlmMessageRole::Assistant,
            content: "prior reply".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: None,
        }]);
        req.model = "no-such-model-9999".into();
        let err =
            llm_request_to_context(&req, "openai").expect_err("unknown model must surface clearly");
        let m = format!("{}", err);
        assert!(m.contains("not in catalog"), "msg: {m}");
        assert!(m.contains("no-such-model-9999"), "msg: {m}");
    }

    #[test]
    fn assistant_history_preserves_reasoning_and_tool_calls() {
        let req = bare_request(vec![LlmMessage {
            role: LlmMessageRole::Assistant,
            content: "calling tool".into(),
            reasoning: Some("considering options".into()),
            tool_calls: Some(vec![crate::types::LlmToolCall {
                id: "call_1".into(),
                tool_type: "function".into(),
                function: crate::types::LlmToolFunction {
                    name: "get_weather".into(),
                    arguments: r#"{"city":"sf"}"#.into(),
                },
            }]),
            tool_call_id: None,
            attachments: None,
        }]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        match &ctx.messages[0] {
            Message::Assistant(am) => {
                assert_eq!(am.content.len(), 3, "thinking + text + tool_call");
                assert!(matches!(am.content[0], model::AssistantContentBlock::Thinking(_)));
                assert!(matches!(am.content[1], model::AssistantContentBlock::Text(_)));
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
    fn tool_result_pulls_tool_name_from_prior_assistant_call() {
        let req = bare_request(vec![
            LlmMessage {
                role: LlmMessageRole::Assistant,
                content: "".into(),
                reasoning: None,
                tool_calls: Some(vec![crate::types::LlmToolCall {
                    id: "call_99".into(),
                    tool_type: "function".into(),
                    function: crate::types::LlmToolFunction {
                        name: "search_web".into(),
                        arguments: r#"{}"#.into(),
                    },
                }]),
                tool_call_id: None,
                attachments: None,
            },
            LlmMessage {
                role: LlmMessageRole::Tool,
                content: "result text".into(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: Some("call_99".into()),
                attachments: None,
            },
        ]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
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
        let req = bare_request(vec![LlmMessage {
            role: LlmMessageRole::Tool,
            content: "orphan".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some("ghost".into()),
            attachments: None,
        }]);
        let err = llm_request_to_context(&req, "openai")
            .expect_err("orphan tool result should error");
        assert!(format!("{}", err).contains("unknown tool_call_id"));
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

    // ---- attachments ---------------------------------------------------

    fn png_attachment() -> crate::types::LlmMessageAttachment {
        crate::types::LlmMessageAttachment {
            name: "screenshot.png".into(),
            mime_type: "image/png".into(),
            data: vec![0x89, 0x50, 0x4e, 0x47], // PNG magic bytes — bytewise not a full image
        }
    }

    fn pdf_attachment() -> crate::types::LlmMessageAttachment {
        crate::types::LlmMessageAttachment {
            name: "doc.pdf".into(),
            mime_type: "application/pdf".into(),
            data: vec![0x25, 0x50, 0x44, 0x46],
        }
    }

    #[test]
    fn image_attachment_translates_to_image_block_before_text() {
        let msg = LlmMessage {
            role: LlmMessageRole::User,
            content: "what is this?".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: Some(vec![png_attachment()]),
        };
        let req = bare_request(vec![msg]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        match &ctx.messages[0] {
            Message::User(um) => match &um.content {
                hand_ai_model::UserContent::Blocks(blocks) => {
                    assert_eq!(blocks.len(), 2);
                    match &blocks[0] {
                        UserContentBlock::Image(img) => {
                            assert_eq!(img.mime_type, "image/png");
                            // Verify base64-encoded PNG magic bytes round-trip.
                            assert_eq!(img.data, "iVBORw==");
                        }
                        other => panic!("expected image first, got {:?}", other),
                    }
                    match &blocks[1] {
                        UserContentBlock::Text(t) => assert_eq!(t.text, "what is this?"),
                        other => panic!("expected text second, got {:?}", other),
                    }
                }
                _ => panic!("expected blocks content for attachment message"),
            },
            other => panic!("expected user message, got {:?}", other),
        }
    }

    #[test]
    fn image_attachment_with_empty_text_omits_text_block() {
        let msg = LlmMessage {
            role: LlmMessageRole::User,
            content: "".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: Some(vec![png_attachment()]),
        };
        let req = bare_request(vec![msg]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        match &ctx.messages[0] {
            Message::User(um) => match &um.content {
                hand_ai_model::UserContent::Blocks(blocks) => {
                    assert_eq!(blocks.len(), 1);
                    assert!(matches!(blocks[0], UserContentBlock::Image(_)));
                }
                _ => panic!("expected blocks content"),
            },
            _ => panic!("expected user message"),
        }
    }

    #[test]
    fn non_image_attachment_returns_validation_error_until_m3() {
        let msg = LlmMessage {
            role: LlmMessageRole::User,
            content: "summarize".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: Some(vec![pdf_attachment()]),
        };
        let req = bare_request(vec![msg]);
        let err = llm_request_to_context(&req, "openai").expect_err("PDF should bounce until M3");
        let msg = format!("{}", err);
        assert!(msg.contains("M3"), "error must signal M3: {}", msg);
        assert!(msg.contains("application/pdf"), "error must mention mime: {}", msg);
    }

    #[test]
    fn empty_attachments_vec_falls_through_to_text_path() {
        let msg = LlmMessage {
            role: LlmMessageRole::User,
            content: "hi".into(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachments: Some(vec![]),
        };
        let req = bare_request(vec![msg]);
        let ctx = llm_request_to_context(&req, "openai").expect("translation");
        match &ctx.messages[0] {
            Message::User(um) => match &um.content {
                hand_ai_model::UserContent::Text(t) => assert_eq!(t, "hi"),
                _ => panic!("empty attachments must not force Blocks variant"),
            },
            _ => panic!("expected user message"),
        }
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
