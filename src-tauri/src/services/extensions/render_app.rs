//! render_app — build a full HTML app shown in the side preview panel.
//!
//! Same presentational contract as render_card: execution only validates and
//! acknowledges, the frontend derives the app's current state by replaying the
//! session's `render_app` toolcall blocks (create sets title+content, update
//! replaces them). Content therefore persists with the transcript for free and
//! survives reloads without any dedicated storage.

use hand_agent::{AgentTool, ToolResult};
use serde_json::json;

use super::VISUAL_DOCTRINE;

/// The render_app tool's fixed name. Gated like `render_card`; the frontend
/// renders toolcall blocks with this name as a pill that opens the right-side
/// app panel (preview + source view).
pub const TOOL_RENDER_APP: &str = "render_app";

/// Hard cap on the `content` argument. Full applications are larger than
/// inline cards, but 1 MB is still far above any legitimate generation.
const RENDER_APP_BYTE_CAP: usize = 1024 * 1024;

/// Message for a `command` that is not `create` / `update`.
const RENDER_APP_INVALID_COMMAND_MSG: &str = "invalid command: must be \"create\" or \"update\"";
/// Message for a missing / non-string / empty `content` argument.
const RENDER_APP_INVALID_CONTENT_MSG: &str = "invalid content argument: must be a non-empty string";
/// Message for a `create` without a usable `title`.
const RENDER_APP_MISSING_TITLE_MSG: &str =
    "invalid title argument: create requires a non-empty title";
/// Message when `content` exceeds [`RENDER_APP_BYTE_CAP`].
const RENDER_APP_TOO_LARGE_MSG: &str = "content too large: exceeds the 1 MB app limit";

/// Build the `render_app` tool.
///
/// Injected per-run via `extra_tools`, alongside `make_render_card_tool`.
pub fn make_render_app_tool() -> AgentTool {
    AgentTool::simple(
        TOOL_RENDER_APP,
        format!(
            "Create or update a complete, self-contained HTML application \
             shown to the user in a side panel with a live preview and a \
             source view. Use it for substantial interactive deliverables — \
             dashboards, games, simulations, full pages — while render_card \
             stays for small inline visual aids. `command: \"create\"` starts \
             a new app (requires title and content); `command: \"update\"` \
             replaces the app's content after user feedback (resend the FULL \
             document, not a diff). `content` must be one complete HTML \
             document with ALL CSS and JavaScript inlined: the panel is a \
             sandboxed iframe with NO network access, so never reference \
             external scripts, stylesheets, fonts, or images.\n\
             \n\
             {VISUAL_DOCTRINE}\n\
             \n\
             The panel is already visible to the user — do not repeat the \
             source code in your text reply."
        ),
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["create", "update"],
                    "description": "create: start a new app; update: replace the current app's content."
                },
                "title": {
                    "type": "string",
                    "description": "Short app title shown in the panel header. Required for create; optional for update (keeps the existing title)."
                },
                "content": {
                    "type": "string",
                    "description": "The complete HTML document (inline CSS/JS only; no external resources)."
                }
            },
            "required": ["command", "content"]
        }),
        "Render app",
        move |_tool_call_id, args| async move { execute_render_app(args) },
    )
}

/// `render_app` body: validate the arguments and acknowledge. The frontend
/// renders the panel from the toolcall block's arguments.
fn execute_render_app(args: serde_json::Value) -> ToolResult {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c @ ("create" | "update")) => c,
        _ => return ToolResult::error(RENDER_APP_INVALID_COMMAND_MSG),
    };

    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) if !c.trim().is_empty() => c,
        _ => return ToolResult::error(RENDER_APP_INVALID_CONTENT_MSG),
    };

    if content.len() > RENDER_APP_BYTE_CAP {
        return ToolResult::error(RENDER_APP_TOO_LARGE_MSG);
    }

    let has_title = args
        .get("title")
        .and_then(|v| v.as_str())
        .is_some_and(|t| !t.trim().is_empty());
    if command == "create" && !has_title {
        return ToolResult::error(RENDER_APP_MISSING_TITLE_MSG);
    }

    match command {
        "create" => ToolResult::text("App created and shown to the user in the preview panel."),
        _ => ToolResult::text("App updated in the preview panel."),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{get_text, tokio_test_block};
    use super::*;

    #[test]
    fn render_app_create_acknowledges() {
        let result = execute_render_app(json!({
            "command": "create",
            "title": "Demo App",
            "content": "<!DOCTYPE html><html><body>hi</body></html>"
        }));
        assert_eq!(
            get_text(&result),
            "App created and shown to the user in the preview panel."
        );
    }

    #[test]
    fn render_app_update_acknowledges_without_title() {
        let result = execute_render_app(json!({
            "command": "update",
            "content": "<!DOCTYPE html><html><body>v2</body></html>"
        }));
        assert_eq!(get_text(&result), "App updated in the preview panel.");
    }

    #[test]
    fn render_app_bad_command_yields_invalid_command_error() {
        // Missing, non-string, and out-of-enum all hit the same guard.
        assert_eq!(
            get_text(&execute_render_app(json!({"content": "<p>x</p>"}))),
            RENDER_APP_INVALID_COMMAND_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": 1, "content": "<p>x</p>"})
            )),
            RENDER_APP_INVALID_COMMAND_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "delete", "content": "<p>x</p>"})
            )),
            RENDER_APP_INVALID_COMMAND_MSG
        );
    }

    #[test]
    fn render_app_bad_content_yields_invalid_content_error() {
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "T"})
            )),
            RENDER_APP_INVALID_CONTENT_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "T", "content": "   "})
            )),
            RENDER_APP_INVALID_CONTENT_MSG
        );
    }

    #[test]
    fn render_app_create_without_title_is_rejected() {
        // Missing, empty, and whitespace-only titles all fail create...
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "content": "<p>x</p>"})
            )),
            RENDER_APP_MISSING_TITLE_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "  ", "content": "<p>x</p>"})
            )),
            RENDER_APP_MISSING_TITLE_MSG
        );
        // ...while update tolerates a missing title (keeps the existing one).
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "update", "content": "<p>x</p>"})
            )),
            "App updated in the preview panel."
        );
    }

    #[test]
    fn render_app_oversized_content_is_rejected() {
        let oversized = "x".repeat(RENDER_APP_BYTE_CAP + 1);
        let result = execute_render_app(json!({
            "command": "update",
            "content": oversized
        }));
        assert_eq!(get_text(&result), RENDER_APP_TOO_LARGE_MSG);
    }

    #[test]
    fn render_app_tool_execute_closure_resolves() {
        let tool = make_render_app_tool();
        assert_eq!(tool.name, TOOL_RENDER_APP);
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-ra".to_string(),
            args: json!({"command": "create", "title": "T", "content": "<b>ok</b>"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let result = tokio_test_block((tool.execute)(ctx)).expect("execute ok");
        assert_eq!(
            get_text(&result),
            "App created and shown to the user in the preview panel."
        );
    }
}
