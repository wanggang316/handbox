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
use futures::Stream;

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChunkResponse, LlmMessage, LlmMessageRole, LlmProvider, LlmRequest, LlmRequestTool,
    LlmResponse,
};

#[allow(unused_imports)] // `model` alias kept for M2 (text/tool-call content blocks)
use hand_ai_model::{
    self as model, AssistantMessageEvent, Context, Message, StopReason, Tool, UserMessage,
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
}

impl HandAiChatClient {
    pub fn new(provider_id: &'static str) -> Self {
        Self { provider_id }
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
        _provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        // M1 stub: as above. The aggregator helper is below; it will be
        // wired up once `stream_simple` is invoked for real.
        let _context = llm_request_to_context(&request)?;
        Err(LlmClientError::validation(
            "hand-ai adapter chat_stream() not yet wired; lands in M2",
        ))
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
// Translation: AssistantMessageEvent stream → LlmChunkResponse stream
// ---------------------------------------------------------------------------

/// Aggregates a stream of hand-ai `AssistantMessageEvent`s into HandBox's
/// completion-style chunk model.
///
/// Not yet wired into `chat_stream()` — sits here for M2 to consume. Lives
/// in this file so the translation contract is co-located with the request
/// translation above.
#[allow(dead_code)] // M2
pub(crate) fn event_to_chunk(
    _event: &AssistantMessageEvent,
    _model_id: &str,
) -> Option<LlmChunkResponse> {
    // M2 will implement the per-variant aggregation defined in
    // exec-plans/hand-ai-integration.md (TextDelta → delta.content,
    // ThinkingDelta → delta.reasoning, ToolCallStart/Delta/End →
    // delta.tool_calls[]). Returns None for variants that do not produce
    // a downstream chunk (e.g. ToolCallStart on its own).
    None
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
