//! render_card — render a self-contained interactive HTML card to the user.
//!
//! PRESENTATIONAL tool: execution only validates the arguments and
//! acknowledges. The frontend renders the card from the toolcall block's
//! `arguments` (which already flow through the message stream and persist with
//! the transcript), so no content is transported through the result and no
//! extra IPC exists. The sandbox iframe on the frontend blocks all external
//! resources; the description teaches the model to inline everything.

use hand_agent::{AgentTool, ToolResult};
use serde_json::json;

use super::VISUAL_DOCTRINE;

/// The render_card tool's fixed name. Injected via `extra_tools` only when the
/// session's `enabled_tools` names it; the frontend special-cases toolcall
/// blocks with this name into an inline sandbox card.
pub const TOOL_RENDER_CARD: &str = "render_card";

/// Hard cap on the `html` argument, protecting the renderer (and the JSONL
/// transcript) from a runaway generation. Cards are small visual aids; 512 KB
/// is far above any legitimate one.
const RENDER_CARD_BYTE_CAP: usize = 512 * 1024;

/// Message for a missing / non-string / empty `html` argument.
const RENDER_CARD_INVALID_ARG_MSG: &str = "invalid html argument: must be a non-empty string";
/// Message when `html` exceeds [`RENDER_CARD_BYTE_CAP`].
const RENDER_CARD_TOO_LARGE_MSG: &str = "html too large: exceeds the 512 KB card limit";

/// Build the `render_card` tool.
///
/// The handler is pure argument validation — see the module docs for why
/// execution carries no content. Injected per-run via `extra_tools` so it
/// needs no `working_dir` and no enablement plumbing.
pub fn make_render_card_tool() -> AgentTool {
    AgentTool::simple(
        TOOL_RENDER_CARD,
        format!(
            "Render a self-contained visual inline in the conversation, \
             directly visible to the user. Use it when a picture explains \
             something better than prose.\n\
             \n\
             FIRST pick the representation that matches the information's \
             structure — never default to a stack of colored boxes:\n\
             - message exchange between parties -> sequence diagram (vertical \
             lifelines, SVG arrows between them)\n\
             - process / decision logic -> flowchart\n\
             - hierarchy / composition -> tree or mindmap\n\
             - phases over time -> timeline or Gantt\n\
             - data entities and relations -> ER / UML boxes with relationship \
             lines\n\
             - cause analysis -> fishbone\n\
             - alternatives -> side-by-side comparison\n\
             - quantities -> SVG bar / line chart\n\
             - none of these fit -> a free-form composition designed for this \
             specific content\n\
             \n\
             {VISUAL_DOCTRINE}\n\
             \n\
             The card has NO frame, title bar or background of its own — it \
             renders directly in the chat flow, so the composition must stand \
             on its own. It must fit the container width (no horizontal \
             overflow) and grow naturally in height; never create inner \
             scroll areas or fixed heights with overflow. It runs in a \
             sandboxed iframe with NO network access: inline ALL CSS and \
             JavaScript, and never reference external scripts, stylesheets, \
             fonts, or images. The card is already shown to the user — do \
             not repeat its content in your text reply."
        ),
        json!({
            "type": "object",
            "properties": {
                "html": {
                    "type": "string",
                    "description": "Self-contained HTML for the card body (inline CSS/JS only; no external resources)."
                },
                "title": {
                    "type": "string",
                    "description": "Optional short title; used as the card's accessible name and hover tooltip, not rendered visibly."
                }
            },
            "required": ["html"]
        }),
        "Render card",
        move |_tool_call_id, args| async move { execute_render_card(args) },
    )
}

/// `render_card` body: validate the arguments and acknowledge. The frontend
/// renders from the toolcall block's arguments, so a success result is a plain
/// acknowledgement string.
fn execute_render_card(args: serde_json::Value) -> ToolResult {
    let html = match args.get("html").and_then(|v| v.as_str()) {
        Some(h) if !h.trim().is_empty() => h,
        _ => return ToolResult::error(RENDER_CARD_INVALID_ARG_MSG),
    };

    if html.len() > RENDER_CARD_BYTE_CAP {
        return ToolResult::error(RENDER_CARD_TOO_LARGE_MSG);
    }

    ToolResult::text("Card rendered to the user.")
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{get_text, tokio_test_block};
    use super::*;

    #[test]
    fn render_card_valid_html_acknowledges() {
        let result = execute_render_card(json!({"html": "<div>hello</div>"}));
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }

    #[test]
    fn render_card_title_is_optional_and_ignored_by_execution() {
        let result = execute_render_card(json!({"html": "<p>x</p>", "title": "Demo"}));
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }

    #[test]
    fn render_card_bad_html_arg_yields_invalid_arg_error() {
        // Missing, non-string, empty, and whitespace-only all hit the same guard.
        assert_eq!(
            get_text(&execute_render_card(json!({}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": 42}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": ""}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": "   "}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
    }

    #[test]
    fn render_card_oversized_html_is_rejected() {
        let oversized = "x".repeat(RENDER_CARD_BYTE_CAP + 1);
        let result = execute_render_card(json!({ "html": oversized }));
        assert_eq!(get_text(&result), RENDER_CARD_TOO_LARGE_MSG);
    }

    /// The tool resolves via the registered `execute` closure (end-to-end
    /// through `AgentTool`), proving the wiring.
    #[test]
    fn render_card_tool_execute_closure_resolves() {
        let tool = make_render_card_tool();
        assert_eq!(tool.name, TOOL_RENDER_CARD);
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-rc".to_string(),
            args: json!({"html": "<b>ok</b>"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let result = tokio_test_block((tool.execute)(ctx)).expect("execute ok");
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }
}
