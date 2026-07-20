//! One-shot LLM session-title generation.
//!
//! Runs a single non-agent completion (no tools, not streamed to the UI)
//! against the session's own model/provider to distill the first user message
//! into a short session title. Used by the `agent_session_generate_title`
//! command for both the auto-on-first-message and the manual right-click paths.

use hand_ai_model::{
    AssistantContentBlock, Client, Context, Message, StopReason, UserMessage,
};

use crate::models::AppError;
use crate::services::model_runtime::{self, ChatOptions};

/// System prompt: produce a bare, short title in the user's own language.
const TITLE_SYSTEM_PROMPT: &str = "You write an extremely short title (at most 6 words, or 16 Chinese characters) that captures what the user wants, based on their first message. Reply with ONLY the title text: no surrounding quotes, no trailing punctuation, no prefix like \"Title:\", no explanation. Use the same language as the user's message.";

/// Output token cap for the title call. Deliberately generous (not ~64): a
/// reasoning model spends part of its output budget on thinking before it
/// emits the title, so a tight cap truncates mid-thought and yields an empty
/// title. 2048 leaves ample room for brief reasoning on a trivial title task
/// plus the short title, while a non-reasoning model still stops early (the cap
/// is never reached, so no extra cost). It also stays within providers that
/// require max_tokens and bound it (e.g. Anthropic-compatible endpoints).
const MAX_OUTPUT_TOKENS: u32 = 2048;

/// Max characters of the source message fed to the model — a guard so a giant
/// first message can't blow the context window or run up cost for a title.
const MAX_SOURCE_CHARS: usize = 2000;

/// Max characters kept from the model's reply as the final title.
const MAX_TITLE_CHARS: usize = 48;

/// Generate a session title from `source_text` using the given provider/model.
///
/// Makes a single `complete_simple` call (tool-less, short output cap) and
/// returns a sanitized one-line title. Errors on a model error or an empty
/// result so the caller surfaces the failure instead of writing a junk name.
pub async fn generate_title(
    provider_type: &str,
    model_id: &str,
    base_url: &str,
    api_key: &str,
    source_text: &str,
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

    let prompt: String = source_text.chars().take(MAX_SOURCE_CHARS).collect();
    let context = Context {
        system_prompt: Some(TITLE_SYSTEM_PROMPT.to_string()),
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

/// Reduce a raw model reply to a clean one-line title: first non-empty line,
/// stripped of wrapping quotes, capped to a sane length.
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
    use super::sanitize_title;

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
