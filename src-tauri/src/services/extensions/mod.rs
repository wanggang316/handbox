//! Extension tools — HandBox-native agent capabilities beyond the coding-agent
//! built-ins.
//!
//! An "extension tool" is a capability HandBox itself adds to an agent run, as
//! opposed to the 7 coding-agent built-ins (`read`/`write`/`edit`/`bash`/
//! `grep`/`find`/`ls`) that `select_enabled_tools` filters. Every extension
//! follows one contract:
//!
//!  - **Identity**: an id on the shared `enabled_tools` list — the same list
//!    the settings default (`agent.defaultEnabledTools`), the per-agent
//!    capability set (`builtinTools`) and per-session edits control.
//!    [`EXTENSION_TOOL_IDS`] is the canonical registry; the frontend mirror is
//!    `src/lib/constants/builtinToolIds.ts`.
//!  - **Activation**: per run, by that id. `web_search` / `render_card` /
//!    `render_app` are constructed here and injected through
//!    `build_agent_session`'s `extra_tools` (agent_run does the gating);
//!    [`TOOL_SKILL`] carries no factory — it gates the coding-agent's own
//!    skill pipeline inside `build_agent_session`.
//!  - **Not a built-in**: `select_enabled_tools` skips these ids silently
//!    instead of warning on them.
//!
//! One module per tool; cross-tool prompt doctrine lives here.

pub mod render_app;
pub mod render_card;
pub mod web_search;

/// The `enabled_tools` id gating the coding-agent skill pipeline. Not a tool
/// factory: when a session's `enabled_tools` names it, `build_agent_session`
/// lets the coding-agent discover skills, index them into the system prompt and
/// register its own `skill` tool; when absent, that whole pipeline is off.
pub const TOOL_SKILL: &str = "skill";

/// The canonical extension-tool id registry, in display order.
///
/// `select_enabled_tools` uses it to skip these ids silently (they are
/// legitimate `enabled_tools` entries resolved outside the built-in filter).
pub const EXTENSION_TOOL_IDS: [&str; 4] = [
    web_search::WEB_SEARCH_TOOL_NAME,
    render_card::TOOL_RENDER_CARD,
    render_app::TOOL_RENDER_APP,
    TOOL_SKILL,
];

/// Visual-quality doctrine shared by the presentational tools' descriptions.
///
/// Models left to their own devices produce "AI slop": emoji icons, one color
/// per item, gradient headers, cramped stacked boxes. These rules are the
/// distilled counter-doctrine (SVG for structure, meaning-bearing accents,
/// hierarchy from spacing not color) and are what makes generated output look
/// designed; keep them in sync across render_card / render_app.
const VISUAL_DOCTRINE: &str = "\
Visual standard — the output must look designed, not generated:\n\
- ABSOLUTELY NO emoji anywhere in the output: not in the title, headings, \
labels, or nodes. Draw all structure (arrows, connectors, nodes, charts) as \
inline SVG; arrows are SVG paths, never unicode characters; if an icon is \
essential, inline a small SVG.\n\
- Restrained palette: neutral surfaces and text from the theme variables, plus \
at most two accent colors that CARRY MEANING (e.g. gray = plaintext, green = \
encrypted). Never one color per item, never rainbow section headers.\n\
- Hierarchy comes from size, weight and spacing — not from more colors. \
Secondary text is muted (~60% opacity); keep text contrast at least 4.5:1.\n\
- Surfaces: hairline 1px borders (var(--hairline)), subtle rounded corners, \
flat or barely tinted fills. No gradients, no heavy shadows, no decorative \
clutter.\n\
- Generous whitespace on an 8px rhythm. When colors or line styles carry \
meaning, end with one small legend line.\n\
Theme variables are provided and adapt to light/dark automatically: \
--base-100/--base-200/--base-300 (surfaces), --base-content (text), \
--hairline (borders), --primary, --info, --success, --warning, --error.";

/// Shared helpers for the extension-tool test modules.
#[cfg(test)]
pub(crate) mod test_support {
    use hand_agent::ToolResult;

    /// Extract the first text content block from a result.
    pub(crate) fn get_text(result: &ToolResult) -> &str {
        match &result.content[0] {
            hand_ai_model::ToolResultContent::Text(t) => &t.text,
            _ => panic!("expected text content"),
        }
    }

    /// Minimal single-thread executor so a tool's async `execute` closure can
    /// be exercised without pulling in a new test-only crate (tokio is already
    /// a dep).
    pub(crate) fn tokio_test_block<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime")
            .block_on(fut)
    }
}
