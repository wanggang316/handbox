//! One-shot LLM session-title generation.
//!
//! A single non-agent completion (no tools, not streamed to the UI) against the
//! session's own model/provider, distilling the source text into a short session
//! title. The source is either the first user message (one-shot naming) or the
//! conversation so far (re-titling as it evolves) — see [`TitleScope`].

use hand_ai_model::{
    AssistantContentBlock, Client, Context, Message, StopReason, UserMessage,
};
use serde::Deserialize;

use crate::models::AppError;
use crate::services::model_runtime::{self, ChatOptions};

/// What the title is distilled from. Only the system prompt differs; the caller
/// builds the matching source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TitleScope {
    /// The session's first user message.
    #[default]
    FirstMessage,
    /// The user messages of the conversation so far, oldest to newest.
    Conversation,
}

const FIRST_MESSAGE_SYSTEM_PROMPT: &str = "You write an extremely short title (at most 6 words, or 16 Chinese characters) that captures what the user wants, based on their first message. Reply with ONLY the title text: no surrounding quotes, no trailing punctuation, no prefix like \"Title:\", no explanation. Use the same language as the user's message.";

const CONVERSATION_SYSTEM_PROMPT: &str = "You write an extremely short title (at most 6 words, or 16 Chinese characters) for a conversation, based on the user's messages listed oldest to newest. Capture what the conversation is about overall, favouring the most recent messages when the topic has shifted. Reply with ONLY the title text: no surrounding quotes, no trailing punctuation, no prefix like \"Title:\", no explanation. Use the same language as the user's messages.";

impl TitleScope {
    fn system_prompt(self) -> &'static str {
        match self {
            Self::FirstMessage => FIRST_MESSAGE_SYSTEM_PROMPT,
            Self::Conversation => CONVERSATION_SYSTEM_PROMPT,
        }
    }
}

/// Deliberately generous: a reasoning model spends part of its output budget
/// thinking before emitting the title, so a tight cap truncates mid-thought into
/// an empty title. A non-reasoning model stops early and never reaches the cap.
const MAX_OUTPUT_TOKENS: u32 = 2048;

/// Guard so a giant source can't blow the context window or run up cost for a
/// title.
const MAX_SOURCE_CHARS: usize = 2000;

const MAX_TITLE_CHARS: usize = 48;

/// Errors on a model error or an empty result, so the caller surfaces the
/// failure instead of writing a junk name.
pub async fn generate_title(
    provider_type: &str,
    model_id: &str,
    base_url: &str,
    api_key: &str,
    source_text: &str,
    scope: TitleScope,
) -> Result<String, AppError> {
    let model = model_runtime::resolve_model(provider_type, model_id, base_url)?;

    let options = model_runtime::build_stream_options(
        &ChatOptions {
            temperature: Some(0.3),
            max_tokens: Some(MAX_OUTPUT_TOKENS),
            ..Default::default()
        },
        api_key,
    );

    let prompt = truncate_source(scope, source_text);
    let context = Context {
        system_prompt: Some(scope.system_prompt().to_string()),
        messages: vec![Message::User(UserMessage::new_text(prompt))],
        tools: None,
    };

    let client = Client::new();
    let message = client
        .complete_simple(&model, context, Some(options))
        .await
        // network_error (not internal_error) so the UI hint is "check your
        // network / provider" rather than the misleading "restart the app".
        .map_err(|e| AppError::network_error(&format!("title generation request failed: {e}")))?;

    // `complete_simple` returns the error message (stop_reason == Error) rather
    // than erroring, so inspect it explicitly.
    if matches!(message.stop_reason, StopReason::Error) {
        let detail = message
            .error_message
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(AppError::network_error(&format!(
            "title generation model error: {detail}"
        )));
    }

    let raw: String = message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();

    let title = sanitize_title(&raw);
    if title.is_empty() {
        return Err(AppError::network_error(
            "the model returned no title text (output may be reasoning-only or truncated)",
        ));
    }
    Ok(title)
}

/// Caps the source at [`MAX_SOURCE_CHARS`], keeping the end for a conversation
/// (the newest messages decide the current topic) and the start for a single
/// first message.
fn truncate_source(scope: TitleScope, source_text: &str) -> String {
    let total = source_text.chars().count();
    if total <= MAX_SOURCE_CHARS {
        return source_text.to_string();
    }
    match scope {
        TitleScope::FirstMessage => source_text.chars().take(MAX_SOURCE_CHARS).collect(),
        TitleScope::Conversation => source_text.chars().skip(total - MAX_SOURCE_CHARS).collect(),
    }
}

fn sanitize_title(raw: &str) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    let trimmed = line
        .trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '`' | '「' | '」' | '“' | '”' | '《' | '》'
            )
        })
        .trim();
    trimmed
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{sanitize_title, truncate_source, TitleScope, MAX_SOURCE_CHARS};

    // The two scopes must not share a prompt: a conversation title is derived
    // from the newest messages, a first-message title from the opening ask.
    #[test]
    fn each_scope_has_its_own_system_prompt() {
        assert_ne!(
            TitleScope::FirstMessage.system_prompt(),
            TitleScope::Conversation.system_prompt()
        );
    }

    // Over-budget sources are cut from opposite ends: the first message keeps
    // its opening, the conversation keeps its most recent turns.
    #[test]
    fn truncation_keeps_the_end_for_a_conversation() {
        let source = format!("{}{}", "a".repeat(MAX_SOURCE_CHARS), "z".repeat(10));

        let first = truncate_source(TitleScope::FirstMessage, &source);
        assert_eq!(first.chars().count(), MAX_SOURCE_CHARS);
        assert!(first.ends_with('a'), "first-message keeps the head");

        let conversation = truncate_source(TitleScope::Conversation, &source);
        assert_eq!(conversation.chars().count(), MAX_SOURCE_CHARS);
        assert!(conversation.ends_with('z'), "conversation keeps the tail");
    }

    // Under-budget sources pass through untouched, whatever the scope, and
    // multi-byte text is counted by chars (never sliced mid-codepoint).
    #[test]
    fn truncation_is_a_noop_within_budget() {
        for scope in [TitleScope::FirstMessage, TitleScope::Conversation] {
            assert_eq!(
                truncate_source(scope, "帮我写一个排序函数"),
                "帮我写一个排序函数"
            );
        }
    }

    #[test]
    fn strips_quotes_and_takes_first_line() {
        assert_eq!(sanitize_title("\"Fix the login bug\""), "Fix the login bug");
        assert_eq!(sanitize_title("  标题第一行 \n 第二行"), "标题第一行");
        assert_eq!(sanitize_title("「翻译请求」"), "翻译请求");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(sanitize_title("   \n  "), "");
    }

    #[test]
    fn caps_length() {
        let long = "a".repeat(100);
        assert_eq!(sanitize_title(&long).chars().count(), super::MAX_TITLE_CHARS);
    }
}
