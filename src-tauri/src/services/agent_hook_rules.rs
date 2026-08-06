//! Evaluates the user's hook rules against the agent's tool calls.
//!
//! Hooks execute actions (run a command, notify); they are not a permission
//! layer — gating what the agent may do belongs to the agent's permission
//! configuration and the [`PermissionExtension`](super::agent_permission::PermissionExtension).
//! Registered BETWEEN the [`SandboxExtension`](super::agent_permission::SandboxExtension)
//! and the approval gate: after the sandbox so a hook can never widen the
//! working-directory boundary, before the gate so a command that vetoes a call
//! spares the user a prompt for something that would be blocked anyway.
//!
//! Rules are a snapshot taken when the session is built. A session is
//! constructed per turn, so an edit in settings takes effect on the next
//! message rather than mid-turn — which also means a rule cannot change under a
//! tool call that is already being judged.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use hand_coding_agent::core::extensions::api::{
    ResultDecision, ToolCallEvent, ToolResultEvent, UserMessageEvent, UserMessageOutcome,
};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};

use crate::services::hook_command::{run_hook_command, CommandSpec, CommandVerdict};
use crate::storage::types::{
    HookAction, HookEvent, HookRule, MatchSubject, DEFAULT_HOOK_COMMAND_TIMEOUT_MS,
};

const RULE_EXTENSION_NAME: &str = "handbox-hook-rules";

/// Tauri event emitted whenever a rule matches — not only for the `notify`
/// action. A rule that silently changes what the agent may do is worse than no
/// rule: the user cannot tell "no rule matched" from "a rule fired and I missed
/// it". Carries `{ sessionId, ruleId, ruleName, action, toolName, outcome,
/// message }`.
pub const HOOK_RULE_NOTIFY_EVENT: &str = "agent_hook_rule_notify";

/// What actually happened, for the notification payload. Distinct from the
/// rule's action because a command resolves several ways.
mod outcome {
    /// A command's verdict blocked the call (or the prompt).
    pub const DENIED: &str = "denied";
    /// A `notify` rule matched.
    pub const OBSERVED: &str = "observed";
    /// A `run_command` hook ran and raised no objection.
    pub const RAN: &str = "ran";
    /// Its command rewrote the tool's arguments.
    pub const REWROTE: &str = "rewrote";
    /// It failed after the call had already run, so nothing could be undone.
    pub const FAILED: &str = "failed";
    /// Its command contributed context for the model to read this turn.
    pub const INFORMED: &str = "informed";
}

/// Sink for [`HOOK_RULE_NOTIFY_EVENT`]. A plain closure so the extension stays
/// free of Tauri types and is testable without a window.
pub type NotifyEmitter = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// Applies the user's rules to each tool call.
pub struct RuleHookExtension {
    manifest: ExtensionManifest,
    /// HandBox DB session id (UUID) — the same key the approval registry and
    /// `abort_run` use. See [`PermissionExtension`](super::agent_permission::PermissionExtension).
    session_id: String,
    notifier: Option<NotifyEmitter>,
    /// Where a `run_command` hook runs. The session's working directory, so a
    /// hook can act on the files the agent is touching without absolute paths.
    working_dir: PathBuf,
    /// Pre-split by event so no hook filters at call time. Each keeps the
    /// repository's `sort_order` — first match decides.
    before_rules: Vec<HookRule>,
    after_rules: Vec<HookRule>,
    prompt_rules: Vec<HookRule>,
}

impl RuleHookExtension {
    pub fn new(session_id: String, rules: Vec<HookRule>) -> Self {
        let mut before_rules = Vec::new();
        let mut after_rules = Vec::new();
        let mut prompt_rules = Vec::new();
        for rule in rules {
            match rule.event {
                HookEvent::BeforeToolCall => before_rules.push(rule),
                HookEvent::AfterToolCall => after_rules.push(rule),
                HookEvent::UserPromptSubmit => prompt_rules.push(rule),
            }
        }

        Self {
            manifest: ExtensionManifest {
                name: RULE_EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Applies the user's declarative hook rules to agent tool calls.".to_string(),
                ),
                capabilities: hand_coding_agent::core::extensions::api::ExtensionCapabilities {
                    before_tool_call: true,
                    after_tool_call: true,
                    // The host ENFORCES this one: an extension that does not
                    // declare it is never called, so it must reflect whether
                    // any prompt rule is actually loaded.
                    on_user_message: !prompt_rules.is_empty(),
                    ..Default::default()
                },
                exec: None,
                env: Default::default(),
                slash_commands: Vec::new(),
                custom_tools: Vec::new(),
                // Tier 1 runs in-process; the host applies these to subprocess RPC only.
                timeouts: Default::default(),
            },
            session_id,
            notifier: None,
            working_dir: std::env::temp_dir(),
            before_rules,
            after_rules,
            prompt_rules,
        }
    }

    /// Where `run_command` hooks run. Defaults to a temp dir so a session built
    /// without one still spawns rather than failing on a missing cwd.
    pub fn with_working_dir(mut self, working_dir: PathBuf) -> Self {
        self.working_dir = working_dir;
        self
    }

    /// Wire the sink match notices are reported through. Without it matches are
    /// only logged.
    pub fn with_notifier(mut self, notifier: Option<NotifyEmitter>) -> Self {
        self.notifier = notifier;
        self
    }

    /// Whether any rule is loaded — lets the caller skip registering an
    /// extension that would do nothing on every tool call.
    pub fn is_empty(&self) -> bool {
        self.before_rules.is_empty() && self.after_rules.is_empty() && self.prompt_rules.is_empty()
    }

    /// Run a `run_command` rule's command against `payload`.
    ///
    /// The payload is built lazily so a rule that somehow reaches here without a
    /// command costs nothing. A missing command denies rather than silently
    /// proceeding: the service layer rejects that combination at write time, so
    /// reaching it means the row was tampered with.
    async fn run_command<F>(
        &self,
        rule: &HookRule,
        event_name: &str,
        tool_name: &str,
        payload: F,
    ) -> CommandVerdict
    where
        F: FnOnce() -> serde_json::Value,
    {
        let Some(command) = rule.command.as_deref().filter(|c| !c.trim().is_empty()) else {
            return CommandVerdict::Deny(format!(
                "hook rule \"{}\" is set to run a command but has none",
                rule.name
            ));
        };

        let timeout = Duration::from_millis(
            rule.timeout_ms
                .filter(|ms| *ms > 0)
                .unwrap_or(DEFAULT_HOOK_COMMAND_TIMEOUT_MS) as u64,
        );

        // The same facts as the JSON, for scripts that would rather branch on an
        // env var than parse stdin.
        let env = vec![
            ("HANDBOX_HOOK_EVENT".to_string(), event_name.to_string()),
            ("HANDBOX_TOOL_NAME".to_string(), tool_name.to_string()),
            ("HANDBOX_SESSION_ID".to_string(), self.session_id.clone()),
            ("HANDBOX_RULE_NAME".to_string(), rule.name.clone()),
        ];

        run_hook_command(
            CommandSpec {
                command,
                working_dir: &self.working_dir,
                timeout,
                env,
            },
            &payload(),
        )
        .await
    }

    /// Report a match to the frontend and the log.
    ///
    /// Every match reports, whatever the action: a rule that quietly blocks or
    /// waves through a tool call leaves the user unable to tell it apart from no
    /// rule matching at all. The log line carries the same facts for the
    /// headless paths, which have no emitter.
    fn report(&self, rule: &HookRule, tool_name: &str, outcome: &str) {
        tracing::info!(
            rule = %rule.name,
            rule_id = %rule.id,
            action = ?rule.action,
            tool = %tool_name,
            outcome = %outcome,
            "[hook_rules] rule matched"
        );

        if let Some(notify) = &self.notifier {
            notify(serde_json::json!({
                "sessionId": self.session_id,
                "ruleId": rule.id,
                "ruleName": rule.name,
                "action": rule.action,
                "toolName": tool_name,
                "outcome": outcome,
                "message": rule.message,
            }));
        }
    }
}

#[async_trait]
impl Extension for RuleHookExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    async fn on_user_message(
        &self,
        _cx: &ExtensionContext,
        event: &UserMessageEvent,
    ) -> Result<UserMessageOutcome, ExtensionError> {
        let Some(rule) = self
            .prompt_rules
            .iter()
            .find(|rule| rule.matches_subject(&MatchSubject::Text(&event.text)))
        else {
            return Ok(UserMessageOutcome::cont());
        };

        // The subject is the prompt, so the "tool" in a notice is the prompt
        // itself; naming it keeps the toast readable.
        const SUBJECT: &str = "prompt";

        match rule.action {
            HookAction::Notify => {
                self.report(rule, SUBJECT, outcome::OBSERVED);
                Ok(UserMessageOutcome::cont())
            }
            HookAction::RunCommand => {
                let verdict = self
                    .run_command(rule, "user_prompt_submit", SUBJECT, || {
                        serde_json::json!({
                            "event": "user_prompt_submit",
                            "sessionId": self.session_id,
                            "ruleId": rule.id,
                            "ruleName": rule.name,
                            "prompt": event.text,
                        })
                    })
                    .await;

                match verdict {
                    // The command's non-decision output becomes context the
                    // model reads for this turn. Upstream attributes it to this
                    // extension and records it as its own message, so it never
                    // masquerades as something the user typed.
                    CommandVerdict::Proceed { context } => {
                        self.report(
                            rule,
                            SUBJECT,
                            if context.is_some() {
                                outcome::INFORMED
                            } else {
                                outcome::RAN
                            },
                        );
                        Ok(match context {
                            Some(text) => UserMessageOutcome::context(text),
                            None => UserMessageOutcome::cont(),
                        })
                    }
                    CommandVerdict::Deny(reason) => {
                        self.report(rule, SUBJECT, outcome::DENIED);
                        Ok(HookDecision::Cancel(reason).into())
                    }
                    // Upstream expects a JSON *string* here, not an object: the
                    // replacement is the prompt text itself.
                    CommandVerdict::ReplaceInput(value) => match value.get("prompt") {
                        Some(serde_json::Value::String(text)) => {
                            self.report(rule, SUBJECT, outcome::REWROTE);
                            Ok(
                                HookDecision::Replace(serde_json::Value::String(text.clone()))
                                    .into(),
                            )
                        }
                        // A rewrite that does not carry a prompt string would be
                        // dropped by the host anyway; treat it as no opinion
                        // rather than silently mangling the turn.
                        _ => {
                            self.report(rule, SUBJECT, outcome::RAN);
                            Ok(UserMessageOutcome::cont())
                        }
                    },
                }
            }
        }
    }

    async fn on_before_tool_call(
        &self,
        _cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        let Some(rule) = self
            .before_rules
            .iter()
            .find(|rule| rule.matches(&event.tool_name, &event.arguments))
        else {
            return Ok(HookDecision::Continue);
        };

        match rule.action {
            HookAction::Notify => {
                self.report(rule, &event.tool_name, outcome::OBSERVED);
                Ok(HookDecision::Continue)
            }
            HookAction::RunCommand => {
                let verdict = self.run_command(rule, "before_tool_call", &event.tool_name, || {
                    serde_json::json!({
                        "event": "before_tool_call",
                        "sessionId": self.session_id,
                        "ruleId": rule.id,
                        "ruleName": rule.name,
                        "toolName": event.tool_name,
                        "callId": event.call_id,
                        "arguments": event.arguments,
                    })
                });

                match verdict.await {
                    // A tool-call hook has no context channel upstream, so any
                    // text the command printed is dropped rather than smuggled
                    // somewhere it does not belong.
                    CommandVerdict::Proceed { .. } => {
                        self.report(rule, &event.tool_name, outcome::RAN);
                        Ok(HookDecision::Continue)
                    }
                    CommandVerdict::Deny(reason) => {
                        self.report(rule, &event.tool_name, outcome::DENIED);
                        Ok(HookDecision::Cancel(reason))
                    }
                    CommandVerdict::ReplaceInput(args) => {
                        self.report(rule, &event.tool_name, outcome::REWROTE);
                        Ok(HookDecision::Replace(args))
                    }
                }
            }
        }
    }

    async fn on_after_tool_call(
        &self,
        _cx: &ExtensionContext,
        event: &ToolResultEvent,
    ) -> Result<ResultDecision, ExtensionError> {
        // After a call there are no arguments left to match on, so the needle is
        // tested against the tool's RESULT.
        let Some(rule) = self
            .after_rules
            .iter()
            .find(|rule| rule.matches(&event.tool_name, &event.result))
        else {
            return Ok(ResultDecision::Continue);
        };

        match rule.action {
            HookAction::Notify => {
                self.report(rule, &event.tool_name, outcome::OBSERVED);
                Ok(ResultDecision::Continue)
            }
            HookAction::RunCommand => {
                // The call already ran, so a verdict cannot undo it. The command
                // runs for its side effect — format the file just written, commit,
                // notify — and a failure is reported, not enforced.
                let verdict = self
                    .run_command(rule, "after_tool_call", &event.tool_name, || {
                        serde_json::json!({
                            "event": "after_tool_call",
                            "sessionId": self.session_id,
                            "ruleId": rule.id,
                            "ruleName": rule.name,
                            "toolName": event.tool_name,
                            "callId": event.call_id,
                            "success": event.success,
                            "result": event.result,
                        })
                    })
                    .await;

                match verdict {
                    // A `Replace` here rewrites what the model reads AND what
                    // the transcript records, which is what makes redaction
                    // actually redact rather than just hide.
                    CommandVerdict::ReplaceInput(result) => {
                        self.report(rule, &event.tool_name, outcome::REWROTE);
                        Ok(ResultDecision::Replace(result))
                    }
                    // The call already ran, so a denial cannot undo it; it is
                    // reported and the result stands.
                    CommandVerdict::Deny(_) => {
                        self.report(rule, &event.tool_name, outcome::FAILED);
                        Ok(ResultDecision::Continue)
                    }
                    CommandVerdict::Proceed { .. } => {
                        self.report(rule, &event.tool_name, outcome::RAN);
                        Ok(ResultDecision::Continue)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    fn cx() -> ExtensionContext {
        ExtensionContext {
            cwd: std::path::PathBuf::from("/tmp"),
            session_id: "coding-agent-internal".to_string(),
            data_dir: std::path::PathBuf::from("/tmp/data"),
        }
    }

    fn rule(name: &str, event: HookEvent, action: HookAction) -> HookRule {
        HookRule {
            id: format!("id-{name}"),
            name: name.to_string(),
            event,
            tool_pattern: "bash".to_string(),
            arg_field: Some("command".to_string()),
            arg_contains: Some("rm -rf".to_string()),
            action,
            message: None,
            command: None,
            timeout_ms: None,
            enabled: true,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn call(tool: &str, args: serde_json::Value) -> ToolCallEvent {
        ToolCallEvent {
            tool_name: tool.to_string(),
            arguments: args,
            call_id: format!("call-{tool}"),
        }
    }

    #[tokio::test]
    async fn a_non_matching_call_passes_through() {
        let ext = RuleHookExtension::new(
            "s1".to_string(),
            vec![HookRule {
                command: Some(r#"echo '{"decision":"deny","reason":"no"}'"#.to_string()),
                ..rule(
                    "block rm",
                    HookEvent::BeforeToolCall,
                    HookAction::RunCommand,
                )
            }],
        );
        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "ls"})))
            .await
            .unwrap();
        assert!(matches!(decision, HookDecision::Continue));
    }

    /// The first rule in `sort_order` decides; a later rule matching the same
    /// call never runs.
    #[tokio::test]
    async fn the_first_matching_rule_wins() {
        let observe = HookRule {
            sort_order: 0,
            ..rule(
                "observe first",
                HookEvent::BeforeToolCall,
                HookAction::Notify,
            )
        };
        let deny = HookRule {
            sort_order: 1,
            command: Some(r#"echo '{"decision":"deny","reason":"no"}'"#.to_string()),
            ..rule(
                "deny second",
                HookEvent::BeforeToolCall,
                HookAction::RunCommand,
            )
        };
        let ext = RuleHookExtension::new("s1".to_string(), vec![observe, deny]);

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /tmp/x"})))
            .await
            .unwrap();
        assert!(
            matches!(decision, HookDecision::Continue),
            "the notify rule sorted first decides the call"
        );
    }

    /// `notify` on a pending call observes it: the call proceeds untouched and
    /// the match is reported.
    #[tokio::test]
    async fn a_notify_rule_observes_a_pending_call() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s3".to_string(),
            vec![rule(
                "watch rm",
                HookEvent::BeforeToolCall,
                HookAction::Notify,
            )],
        )
        .with_notifier(Some(notifier));

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();

        assert!(matches!(decision, HookDecision::Continue));
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["outcome"], "observed");
    }

    /// A block must announce itself. Without this the user cannot tell a rule
    /// firing apart from no rule matching — the failure that made the feature
    /// look broken in real use.
    #[tokio::test]
    async fn a_command_deny_reports_the_block() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s8".to_string(),
            vec![HookRule {
                command: Some(r#"echo '{"decision":"deny","reason":"no"}'"#.to_string()),
                ..rule(
                    "block rm",
                    HookEvent::BeforeToolCall,
                    HookAction::RunCommand,
                )
            }],
        )
        .with_notifier(Some(notifier));

        ext.on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1, "a block must surface");
        assert_eq!(events[0]["outcome"], "denied");
        assert_eq!(events[0]["ruleName"], "block rm");
        assert_eq!(events[0]["toolName"], "bash");
    }

    /// A call no rule matched stays silent, or every tool call would toast.
    #[tokio::test]
    async fn a_non_matching_call_reports_nothing() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s9".to_string(),
            vec![rule(
                "watch rm",
                HookEvent::BeforeToolCall,
                HookAction::Notify,
            )],
        )
        .with_notifier(Some(notifier));

        ext.on_before_tool_call(&cx(), &call("bash", json!({"command": "ls"})))
            .await
            .unwrap();

        assert!(seen.lock().unwrap().is_empty());
    }

    fn prompt(text: &str) -> UserMessageEvent {
        UserMessageEvent {
            text: text.to_string(),
        }
    }

    /// A prompt rule matches the prompt TEXT — there is no tool name to glob,
    /// so a rule written for prompts must not be filtered out by the pattern.
    #[tokio::test]
    async fn a_prompt_rule_matches_the_prompt_text() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s15".to_string(),
            vec![HookRule {
                event: HookEvent::UserPromptSubmit,
                tool_pattern: "bash".to_string(), // deliberately irrelevant
                arg_field: None,
                arg_contains: Some("deploy to prod".to_string()),
                action: HookAction::RunCommand,
                command: Some(r#"echo '{"decision":"deny","reason":"not from here"}'"#.to_string()),
                ..rule("guard", HookEvent::UserPromptSubmit, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let blocked = ext
            .on_user_message(&cx(), &prompt("please deploy to prod now"))
            .await
            .unwrap();
        match blocked.decision {
            HookDecision::Cancel(reason) => assert!(reason.contains("not from here")),
            other => panic!("expected Cancel, got {other:?}"),
        }

        let allowed = ext
            .on_user_message(&cx(), &prompt("run the tests"))
            .await
            .unwrap();
        assert!(matches!(allowed.decision, HookDecision::Continue));
    }

    /// The capability is host-ENFORCED: without it the hook is never dispatched,
    /// so it must track whether a prompt rule is actually loaded.
    #[test]
    fn the_user_message_capability_tracks_whether_prompt_rules_exist() {
        let without = RuleHookExtension::new(
            "s16".to_string(),
            vec![rule("tool", HookEvent::BeforeToolCall, HookAction::Notify)],
        );
        assert!(!without.manifest().capabilities.on_user_message);

        let with = RuleHookExtension::new(
            "s17".to_string(),
            vec![rule(
                "prompt",
                HookEvent::UserPromptSubmit,
                HookAction::Notify,
            )],
        );
        assert!(with.manifest().capabilities.on_user_message);
    }

    /// Upstream expects the replacement prompt as a JSON string, so the command
    /// returns it under `prompt` and we unwrap it.
    #[tokio::test]
    async fn a_prompt_command_can_rewrite_the_prompt() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s18".to_string(),
            vec![HookRule {
                event: HookEvent::UserPromptSubmit,
                arg_field: None,
                arg_contains: None,
                action: HookAction::RunCommand,
                command: Some(
                    r#"echo '{"updatedInput":{"prompt":"rewritten prompt"}}'"#.to_string(),
                ),
                ..rule(
                    "inject",
                    HookEvent::UserPromptSubmit,
                    HookAction::RunCommand,
                )
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_user_message(&cx(), &prompt("original"))
            .await
            .unwrap();

        match decision.decision {
            HookDecision::Replace(value) => {
                assert_eq!(value, serde_json::Value::String("rewritten prompt".into()))
            }
            other => panic!("expected Replace, got {other:?}"),
        }
    }

    /// The point of the whole action: a hook that DOES something. The command
    /// runs in the session's working directory with the event on stdin.
    #[tokio::test]
    async fn a_run_command_rule_executes_its_command() {
        let dir = tempfile::TempDir::new().unwrap();
        let marker = dir.path().join("hook-ran.txt");

        let ext = RuleHookExtension::new(
            "s10".to_string(),
            vec![HookRule {
                action: HookAction::RunCommand,
                command: Some(format!("cat > {}", marker.display())),
                ..rule("on bash", HookEvent::BeforeToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /tmp/x"})))
            .await
            .unwrap();

        assert!(
            matches!(decision, HookDecision::Continue),
            "a command that says nothing lets the call through"
        );
        let seen: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(seen["event"], "before_tool_call");
        assert_eq!(seen["toolName"], "bash");
        assert_eq!(seen["arguments"]["command"], "rm -rf /tmp/x");
    }

    /// A command can veto a pending call through its verdict — the dynamic
    /// escape hatch that replaced the declarative deny action.
    #[tokio::test]
    async fn a_run_command_rule_can_deny() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s11".to_string(),
            vec![HookRule {
                action: HookAction::RunCommand,
                command: Some(
                    r#"echo '{"decision":"deny","reason":"policy says no"}'"#.to_string(),
                ),
                ..rule("gate", HookEvent::BeforeToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();

        match decision {
            HookDecision::Cancel(reason) => assert_eq!(reason, "policy says no"),
            other => panic!("expected Cancel, got {other:?}"),
        }
    }

    /// `updatedInput` rewrites the arguments the tool actually runs with.
    #[tokio::test]
    async fn a_run_command_rule_can_rewrite_the_arguments() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s12".to_string(),
            vec![HookRule {
                action: HookAction::RunCommand,
                command: Some(r#"echo '{"updatedInput":{"command":"ls"}}'"#.to_string()),
                ..rule("rewrite", HookEvent::BeforeToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();

        match decision {
            HookDecision::Replace(args) => assert_eq!(args, json!({"command": "ls"})),
            other => panic!("expected Replace, got {other:?}"),
        }
    }

    /// After the call the command still runs — that is the auto-format /
    /// auto-commit case — but its verdict cannot un-run anything.
    #[tokio::test]
    async fn an_after_call_command_runs_for_its_side_effect() {
        let dir = tempfile::TempDir::new().unwrap();
        let marker = dir.path().join("after.txt");

        let ext = RuleHookExtension::new(
            "s13".to_string(),
            vec![HookRule {
                tool_pattern: "write".to_string(),
                arg_field: None,
                arg_contains: None,
                action: HookAction::RunCommand,
                command: Some(format!("cat > {}", marker.display())),
                ..rule("format", HookEvent::AfterToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        ext.on_after_tool_call(
            &cx(),
            &ToolResultEvent {
                tool_name: "write".to_string(),
                call_id: "c1".to_string(),
                success: true,
                result: json!({"path": "/repo/a.rs"}),
            },
        )
        .await
        .unwrap();

        let seen: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&marker).unwrap()).unwrap();
        assert_eq!(seen["event"], "after_tool_call");
        assert_eq!(seen["success"], true);
        assert_eq!(seen["result"]["path"], "/repo/a.rs");
    }

    /// The case #147 was filed for: tell the model something without spending
    /// the turn. The prompt is untouched and the context rides alongside it.
    #[tokio::test]
    async fn a_prompt_command_can_contribute_context() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s21".to_string(),
            vec![HookRule {
                event: HookEvent::UserPromptSubmit,
                arg_field: None,
                arg_contains: None,
                action: HookAction::RunCommand,
                command: Some("echo 'on branch main, 3 files dirty'".to_string()),
                ..rule("state", HookEvent::UserPromptSubmit, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let outcome = ext
            .on_user_message(&cx(), &prompt("fix the build"))
            .await
            .unwrap();

        assert!(
            matches!(outcome.decision, HookDecision::Continue),
            "informing the model must not cost the turn"
        );
        assert_eq!(
            outcome.additional_context.as_deref(),
            Some("on branch main, 3 files dirty")
        );
    }

    /// A command that only acts contributes nothing, so a hook that writes a
    /// file does not also whisper at the model.
    #[tokio::test]
    async fn a_silent_prompt_command_contributes_no_context() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s22".to_string(),
            vec![HookRule {
                event: HookEvent::UserPromptSubmit,
                arg_field: None,
                arg_contains: None,
                action: HookAction::RunCommand,
                command: Some("true".to_string()),
                ..rule("quiet", HookEvent::UserPromptSubmit, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let outcome = ext.on_user_message(&cx(), &prompt("hi")).await.unwrap();

        assert!(matches!(outcome.decision, HookDecision::Continue));
        assert_eq!(outcome.additional_context, None);
    }

    /// v0.4.2 made the after hook able to rewrite a result, so a command can
    /// now redact what the model reads — and, per upstream, what the transcript
    /// records. Before this the verdict had nowhere to go.
    #[tokio::test]
    async fn an_after_call_command_can_rewrite_the_result() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s19".to_string(),
            vec![HookRule {
                tool_pattern: "read".to_string(),
                arg_field: None,
                arg_contains: Some("sk-live".to_string()),
                action: HookAction::RunCommand,
                command: Some(r#"echo '{"updatedInput":{"content":"[redacted]"}}'"#.to_string()),
                ..rule("scrub", HookEvent::AfterToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_after_tool_call(
                &cx(),
                &ToolResultEvent {
                    tool_name: "read".to_string(),
                    call_id: "c1".to_string(),
                    success: true,
                    result: json!({"content": "token sk-live-abc123"}),
                },
            )
            .await
            .unwrap();

        match decision {
            ResultDecision::Replace(value) => {
                assert_eq!(value, json!({"content": "[redacted]"}));
            }
            other => panic!("expected Replace, got {other:?}"),
        }
    }

    /// A failure after the call cannot undo it, so the tool's own result stands.
    #[tokio::test]
    async fn a_failing_after_call_command_leaves_the_result_alone() {
        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s20".to_string(),
            vec![HookRule {
                tool_pattern: "*".to_string(),
                arg_field: None,
                arg_contains: None,
                action: HookAction::RunCommand,
                command: Some("exit 1".to_string()),
                ..rule("broken", HookEvent::AfterToolCall, HookAction::RunCommand)
            }],
        )
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext
            .on_after_tool_call(
                &cx(),
                &ToolResultEvent {
                    tool_name: "read".to_string(),
                    call_id: "c1".to_string(),
                    success: true,
                    result: json!({"content": "kept"}),
                },
            )
            .await
            .unwrap();

        assert!(matches!(decision, ResultDecision::Continue));
    }

    /// A rule set to run a command but carrying none denies rather than quietly
    /// doing nothing — the write path rejects this, so seeing it means the row
    /// was tampered with.
    #[tokio::test]
    async fn a_command_rule_without_a_command_denies() {
        let ext = RuleHookExtension::new(
            "s14".to_string(),
            vec![HookRule {
                action: HookAction::RunCommand,
                command: None,
                ..rule("broken", HookEvent::BeforeToolCall, HookAction::RunCommand)
            }],
        );

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    /// `notify` fires on the result rather than the arguments, since a finished
    /// call no longer carries them.
    #[tokio::test]
    async fn a_notify_rule_reports_a_matching_result() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s5".to_string(),
            vec![HookRule {
                tool_pattern: "*".to_string(),
                arg_field: None,
                arg_contains: Some("secret".to_string()),
                ..rule("watch", HookEvent::AfterToolCall, HookAction::Notify)
            }],
        )
        .with_notifier(Some(notifier));

        ext.on_after_tool_call(
            &cx(),
            &ToolResultEvent {
                tool_name: "read".to_string(),
                call_id: "c1".to_string(),
                success: true,
                result: json!({"content": "a secret value"}),
            },
        )
        .await
        .unwrap();

        ext.on_after_tool_call(
            &cx(),
            &ToolResultEvent {
                tool_name: "read".to_string(),
                call_id: "c2".to_string(),
                success: true,
                result: json!({"content": "nothing here"}),
            },
        )
        .await
        .unwrap();

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1, "only the matching result notifies");
        assert_eq!(events[0]["ruleName"], "watch");
        assert_eq!(events[0]["toolName"], "read");
    }

    /// Before-rules and after-rules are split at construction, so a rule stored
    /// for one event never fires on the other.
    #[tokio::test]
    async fn rules_only_fire_on_their_own_event() {
        let ext = RuleHookExtension::new(
            "s6".to_string(),
            vec![HookRule {
                command: Some(r#"echo '{"decision":"deny","reason":"no"}'"#.to_string()),
                ..rule(
                    "after only",
                    HookEvent::AfterToolCall,
                    HookAction::RunCommand,
                )
            }],
        );
        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();
        assert!(
            matches!(decision, HookDecision::Continue),
            "an after-rule must not decide a pending call"
        );
    }
}
