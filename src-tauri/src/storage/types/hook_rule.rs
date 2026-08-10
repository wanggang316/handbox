// Declarative hook-rule domain types.
//
// Mirrors the `agent_hook_rules` table (migration 063). Matching lives here as a
// pure function over `(tool_name, arguments)` so the dispatch path in
// `services/agent_hook_rules` stays a thin loop, and so the semantics are
// testable without a live session.

use serde::{Deserialize, Serialize};

use super::{Timestamp, UUID};

/// Which point of the agent's lifecycle a rule fires at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    BeforeToolCall,
    AfterToolCall,
    /// The user submitted a prompt, before it enters the transcript. Matching
    /// is against the prompt text rather than a tool call, and the rule can
    /// rewrite or refuse the turn.
    UserPromptSubmit,
    /// The agent finished its reply, before control returns to the user.
    /// Matching is against the assistant's final text; a command's deny
    /// verdict sends the agent back to work with the reason as instruction.
    TurnEnd,
    /// A tool call paused for the user's approval. Matching is against the
    /// call like `BeforeToolCall`, but the actions are report-only — the
    /// decision stays with the user.
    ApprovalRequested,
}

/// What a rule is matched against, which differs by event.
pub enum MatchSubject<'a> {
    /// A tool call: glob the name, then optionally test an argument.
    Tool {
        name: &'a str,
        arguments: &'a serde_json::Value,
    },
    /// Free text (a prompt). The tool pattern does not apply; only the
    /// substring test does, against the text itself.
    Text(&'a str),
}

/// What a matching rule does.
///
/// Hooks execute actions; they are not a permission layer. Gating what the
/// agent may do belongs to the agent's own permission configuration, so the
/// declarative decision actions (deny/ask/allow) were removed. A command can
/// still veto or rewrite through its verdict — that is the dynamic escape
/// hatch, not the feature's point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookAction {
    /// Emit an event to the frontend; never changes the outcome.
    Notify,
    /// Run the rule's command, feeding it the event on stdin. Its output may
    /// contribute context, rewrite arguments or results, or deny the call.
    RunCommand,
}

/// Default budget for a [`HookAction::RunCommand`] hook, in milliseconds.
///
/// Deliberately short: this runs inline with a tool call, so a slow hook is
/// felt on every turn. Formatters and notifiers finish well inside it; anything
/// genuinely long-running should be started detached by the command itself.
pub const DEFAULT_HOOK_COMMAND_TIMEOUT_MS: i64 = 10_000;

/// One user-authored rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRule {
    pub id: UUID,
    pub name: String,
    pub event: HookEvent,
    /// Tool-name glob — see [`glob_matches`].
    pub tool_pattern: String,
    /// Argument to inspect. `None` matches against the whole arguments object.
    pub arg_field: Option<String>,
    /// Substring the argument must contain. `None`/empty matches on the tool
    /// pattern alone.
    pub arg_contains: Option<String>,
    pub action: HookAction,
    /// Shown to the user alongside the match notice.
    pub message: Option<String>,
    /// Shell command for [`HookAction::RunCommand`]; ignored by the others.
    pub command: Option<String>,
    /// Budget for the command in milliseconds; `None` uses
    /// [`DEFAULT_HOOK_COMMAND_TIMEOUT_MS`].
    pub timeout_ms: Option<i64>,
    pub enabled: bool,
    /// Evaluation order; the first matching rule decides the call.
    pub sort_order: i64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateHookRuleRequest {
    pub name: String,
    pub event: HookEvent,
    pub tool_pattern: String,
    #[serde(default)]
    pub arg_field: Option<String>,
    #[serde(default)]
    pub arg_contains: Option<String>,
    pub action: HookAction,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<i64>,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

/// Every field optional: an omitted field keeps the stored value, so the
/// frontend can PATCH a single toggle without shipping the whole rule back.
///
/// For the three nullable columns ([`Self::arg_field`], [`Self::arg_contains`],
/// [`Self::message`]) an **empty string clears** the column — a flat `Option`
/// cannot distinguish "leave alone" from "set to null" otherwise, and the
/// alternative (nested options) would pull in a dependency for one field shape.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHookRuleRequest {
    pub name: Option<String>,
    pub event: Option<HookEvent>,
    pub tool_pattern: Option<String>,
    pub arg_field: Option<String>,
    pub arg_contains: Option<String>,
    pub action: Option<HookAction>,
    pub message: Option<String>,
    pub command: Option<String>,
    pub timeout_ms: Option<i64>,
    pub enabled: Option<bool>,
    pub sort_order: Option<i64>,
}

impl HookRule {
    /// Whether this rule applies to one tool call.
    ///
    /// Enablement and event are filtered by the query that loads the rules, so
    /// this is purely the pattern side: tool glob first, then the optional
    /// substring test against either one named argument or the whole object.
    ///
    /// A rule naming an argument the call does not carry does NOT match — the
    /// safer reading, since a rule written for `bash.command` should not fire on
    /// an unrelated tool that happens to have been caught by a loose glob.
    pub fn matches(&self, tool_name: &str, arguments: &serde_json::Value) -> bool {
        self.matches_subject(&MatchSubject::Tool {
            name: tool_name,
            arguments,
        })
    }

    /// Whether this rule applies, for whichever subject its event carries.
    pub fn matches_subject(&self, subject: &MatchSubject<'_>) -> bool {
        // The tool glob only means something for a tool call; a prompt has no
        // name to match, so the pattern is skipped rather than failing it.
        if let MatchSubject::Tool { name, .. } = subject {
            if !glob_matches(&self.tool_pattern, name) {
                return false;
            }
        }

        let Some(needle) = self
            .arg_contains
            .as_deref()
            .filter(|needle| !needle.is_empty())
        else {
            return true;
        };

        let haystack = match subject {
            MatchSubject::Text(text) => (*text).to_string(),
            MatchSubject::Tool { arguments, .. } => {
                match self.arg_field.as_deref().filter(|f| !f.is_empty()) {
                    Some(field) => match arguments.get(field) {
                        // Strings compare raw so the user's needle doesn't have
                        // to account for JSON quoting; anything else compares
                        // serialized.
                        Some(serde_json::Value::String(text)) => text.clone(),
                        Some(other) => other.to_string(),
                        None => return false,
                    },
                    None => arguments.to_string(),
                }
            }
        };

        haystack.contains(needle)
    }
}

/// `*` is the only wildcard: a bare `*` matches everything, and a single leading
/// or trailing `*` anchors the other end.
///
/// Deliberately not regex. It covers what tool names actually need (`bash`,
/// `mcp__*`, `*`), keeps a mistyped rule from silently matching nothing, and
/// leaves no regex denial-of-service surface in a path that runs on every tool
/// call.
fn glob_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rule(tool_pattern: &str, arg_field: Option<&str>, arg_contains: Option<&str>) -> HookRule {
        HookRule {
            id: "r1".to_string(),
            name: "test".to_string(),
            event: HookEvent::BeforeToolCall,
            tool_pattern: tool_pattern.to_string(),
            arg_field: arg_field.map(str::to_string),
            arg_contains: arg_contains.map(str::to_string),
            action: HookAction::Notify,
            message: None,
            command: None,
            timeout_ms: None,
            enabled: true,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn bare_star_matches_every_tool() {
        assert!(glob_matches("*", "bash"));
        assert!(glob_matches("*", "mcp__github__create_issue"));
    }

    #[test]
    fn prefix_and_suffix_globs_anchor_the_other_end() {
        assert!(glob_matches("mcp__*", "mcp__github__create_issue"));
        assert!(!glob_matches("mcp__*", "bash"));
        assert!(glob_matches("*_issue", "mcp__github__create_issue"));
        assert!(!glob_matches("*_issue", "bash"));
    }

    #[test]
    fn a_literal_pattern_is_exact() {
        assert!(glob_matches("bash", "bash"));
        assert!(!glob_matches("bash", "bash_extra"));
        assert!(!glob_matches("bash", "run_bash"));
    }

    /// Tool pattern alone is a complete rule.
    #[test]
    fn no_needle_matches_on_the_tool_pattern_alone() {
        assert!(rule("bash", None, None).matches("bash", &json!({"command": "ls"})));
        assert!(rule("bash", None, Some("")).matches("bash", &json!({"command": "ls"})));
    }

    #[test]
    fn named_field_is_matched_as_a_substring() {
        let r = rule("bash", Some("command"), Some("rm -rf"));
        assert!(r.matches("bash", &json!({"command": "rm -rf /tmp/x"})));
        assert!(!r.matches("bash", &json!({"command": "ls -la"})));
    }

    /// The safer reading: a rule about an argument that isn't there does not
    /// fire, rather than falling back to scanning the whole object.
    #[test]
    fn a_missing_named_field_does_not_match() {
        let r = rule("*", Some("command"), Some("rm -rf"));
        assert!(!r.matches("write", &json!({"path": "/tmp/rm -rf"})));
    }

    /// With no field named, the needle is searched across the whole object, so a
    /// rule can catch a value without knowing which parameter carries it.
    #[test]
    fn without_a_field_the_whole_argument_object_is_searched() {
        let r = rule("*", None, Some(".env"));
        assert!(r.matches("read", &json!({"path": "/repo/.env"})));
        assert!(r.matches("write", &json!({"file_path": "/repo/.env", "content": "x"})));
        assert!(!r.matches("read", &json!({"path": "/repo/README.md"})));
    }

    /// A non-string field compares against its serialized form, so numeric and
    /// boolean arguments are still reachable.
    #[test]
    fn non_string_fields_compare_serialized() {
        let r = rule("*", Some("count"), Some("42"));
        assert!(r.matches("tool", &json!({"count": 42})));
        assert!(!r.matches("tool", &json!({"count": 7})));
    }

    /// A string field is compared raw: the user writes `rm -rf`, not `\"rm -rf\"`.
    #[test]
    fn string_fields_compare_without_json_quoting() {
        let r = rule("bash", Some("command"), Some("\""));
        assert!(
            !r.matches("bash", &json!({"command": "ls"})),
            "the quotes JSON would add must not be part of the haystack"
        );
    }
}
