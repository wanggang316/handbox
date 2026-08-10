//! Evaluates the user's hook rules against the agent's lifecycle: tool calls,
//! prompt submission, turn end, and the approval pause.
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
    ResultDecision, ToolCallEvent, ToolResultEvent, TurnEndEvent, UserMessageEvent,
    UserMessageOutcome,
};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};

use crate::services::agent_permission::ApprovalEmitter;
use crate::services::hook_command::{run_hook_command, CommandRun, CommandSpec, CommandVerdict};
use crate::storage::types::{
    HookAction, HookEvent, HookRule, MatchSubject, DEFAULT_HOOK_COMMAND_TIMEOUT_MS,
};

const RULE_EXTENSION_NAME: &str = "handbox-hook-rules";

/// Tauri event emitted whenever a rule matches — not only for the `notify`
/// action. A rule that silently changes what the agent may do is worse than no
/// rule: the user cannot tell "no rule matched" from "a rule fired and I missed
/// it". Carries `{ sessionId, ruleId, ruleName, action, event, toolName,
/// callId, outcome, message, detail }` — `callId` lets the timeline attach the
/// entry to its tool card, `detail` is a command's execution capture.
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
    /// Its command broke — or, on a report-only event, objected — with nothing
    /// left to enforce.
    pub const FAILED: &str = "failed";
    /// Its command contributed context for the model to read this turn.
    pub const INFORMED: &str = "informed";
    /// Its command refused to let the turn end, so the agent keeps working.
    pub const RESUMED: &str = "resumed";
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
    turn_end_rules: Vec<HookRule>,
    /// Not dispatched through the [`Extension`] trait: the approval pause is
    /// HandBox's own, so these fire through [`wrap_approval_emitter`].
    approval_rules: Vec<HookRule>,
}

impl RuleHookExtension {
    pub fn new(session_id: String, rules: Vec<HookRule>) -> Self {
        let mut before_rules = Vec::new();
        let mut after_rules = Vec::new();
        let mut prompt_rules = Vec::new();
        let mut turn_end_rules = Vec::new();
        let mut approval_rules = Vec::new();
        for rule in rules {
            match rule.event {
                HookEvent::BeforeToolCall => before_rules.push(rule),
                HookEvent::AfterToolCall => after_rules.push(rule),
                HookEvent::UserPromptSubmit => prompt_rules.push(rule),
                HookEvent::TurnEnd => turn_end_rules.push(rule),
                HookEvent::ApprovalRequested => approval_rules.push(rule),
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
                    // The host ENFORCES these two: an extension that does not
                    // declare them is never called, so each must reflect
                    // whether any such rule is actually loaded.
                    on_user_message: !prompt_rules.is_empty(),
                    on_turn_end: !turn_end_rules.is_empty(),
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
            turn_end_rules,
            approval_rules,
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

    /// Whether any rule needs the [`Extension`] dispatch chain — lets the
    /// caller skip registering an extension that would do nothing on every
    /// tool call. Approval rules don't count: they fire through the wrapped
    /// emitter, not the chain.
    pub fn has_extension_rules(&self) -> bool {
        !self.before_rules.is_empty()
            || !self.after_rules.is_empty()
            || !self.prompt_rules.is_empty()
            || !self.turn_end_rules.is_empty()
    }

    /// Whether any rule fires on the approval pause — decides if the approval
    /// emitter is worth wrapping at all.
    pub fn has_approval_rules(&self) -> bool {
        !self.approval_rules.is_empty()
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
    ) -> CommandRun
    where
        F: FnOnce() -> serde_json::Value,
    {
        let Some(command) = rule.command.as_deref().filter(|c| !c.trim().is_empty()) else {
            return CommandRun {
                verdict: CommandVerdict::Deny(format!(
                    "hook rule \"{}\" is set to run a command but has none",
                    rule.name
                )),
                detail: None,
            };
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
    ///
    /// `call_id` ties a tool-call firing to its card in the timeline (`None`
    /// for prompt rules); `detail` is a command's execution capture.
    fn report(
        &self,
        rule: &HookRule,
        tool_name: &str,
        outcome: &str,
        call_id: Option<&str>,
        detail: Option<&str>,
    ) {
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
                "event": rule.event,
                "toolName": tool_name,
                "callId": call_id,
                "outcome": outcome,
                "message": rule.message,
                "detail": detail,
            }));
        }
    }

    /// A tool call paused for the user's approval. Not an [`Extension`] hook —
    /// the pause is HandBox's own (see [`wrap_approval_emitter`]) — and
    /// strictly REPORT-ONLY: the decision belongs to the user and the
    /// permission system, so a command runs for its side effect (ring a bell,
    /// push to a phone) and its verdict enforces nothing.
    pub async fn on_approval_requested(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
        call_id: &str,
    ) {
        let Some(rule) = self
            .approval_rules
            .iter()
            .find(|rule| rule.matches(tool_name, arguments))
        else {
            return;
        };

        match rule.action {
            HookAction::Notify => {
                self.report(rule, tool_name, outcome::OBSERVED, Some(call_id), None);
            }
            HookAction::RunCommand => {
                let run = self
                    .run_command(rule, "approval_requested", tool_name, || {
                        serde_json::json!({
                            "event": "approval_requested",
                            "sessionId": self.session_id,
                            "ruleId": rule.id,
                            "ruleName": rule.name,
                            "toolName": tool_name,
                            "callId": call_id,
                            "arguments": arguments,
                        })
                    })
                    .await;
                let detail = run.detail.as_deref();

                let outcome = match run.verdict {
                    CommandVerdict::Proceed { .. } | CommandVerdict::ReplaceInput(_) => {
                        outcome::RAN
                    }
                    // An objection with nothing to enforce reads the same as a
                    // breakage: the request stays with the user either way.
                    CommandVerdict::Deny(_) | CommandVerdict::Errored(_) => outcome::FAILED,
                };
                self.report(rule, tool_name, outcome, Some(call_id), detail);
            }
        }
    }
}

/// Interpose the user's approval-requested rules on the approval channel.
///
/// The emitter fires exactly when a call actually pauses for the user — an
/// always-allowed tool never emits, so a hook here never fires for it. The
/// rules run on a spawned task so a slow command cannot delay the approval
/// prompt itself, and the payload is forwarded to `inner` untouched.
pub fn wrap_approval_emitter(
    rules: Arc<RuleHookExtension>,
    inner: ApprovalEmitter,
) -> ApprovalEmitter {
    Arc::new(move |payload: serde_json::Value| {
        // Field names follow the emitted approval payload (`agent_permission`).
        let tool_name = payload["toolName"].as_str().unwrap_or_default().to_string();
        let call_id = payload["callId"].as_str().unwrap_or_default().to_string();
        let arguments = payload["args"].clone();
        let rules = Arc::clone(&rules);
        tokio::spawn(async move {
            rules
                .on_approval_requested(&tool_name, &arguments, &call_id)
                .await;
        });
        inner(payload);
    })
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
                self.report(rule, SUBJECT, outcome::OBSERVED, None, None);
                Ok(UserMessageOutcome::cont())
            }
            HookAction::RunCommand => {
                let run = self
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
                let detail = run.detail.as_deref();

                match run.verdict {
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
                            None,
                            detail,
                        );
                        Ok(match context {
                            Some(text) => UserMessageOutcome::context(text),
                            None => UserMessageOutcome::cont(),
                        })
                    }
                    CommandVerdict::Deny(reason) => {
                        self.report(rule, SUBJECT, outcome::DENIED, None, detail);
                        Ok(HookDecision::Cancel(reason).into())
                    }
                    // Fail closed, like the pending-call hooks: a gate that
                    // broke did not consent to the turn.
                    CommandVerdict::Errored(reason) => {
                        self.report(rule, SUBJECT, outcome::FAILED, None, detail);
                        Ok(HookDecision::Cancel(reason).into())
                    }
                    // Upstream expects a JSON *string* here, not an object: the
                    // replacement is the prompt text itself.
                    CommandVerdict::ReplaceInput(value) => match value.get("prompt") {
                        Some(serde_json::Value::String(text)) => {
                            self.report(rule, SUBJECT, outcome::REWROTE, None, detail);
                            Ok(
                                HookDecision::Replace(serde_json::Value::String(text.clone()))
                                    .into(),
                            )
                        }
                        // A rewrite that does not carry a prompt string would be
                        // dropped by the host anyway; treat it as no opinion
                        // rather than silently mangling the turn.
                        _ => {
                            self.report(rule, SUBJECT, outcome::RAN, None, detail);
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
                self.report(
                    rule,
                    &event.tool_name,
                    outcome::OBSERVED,
                    Some(&event.call_id),
                    None,
                );
                Ok(HookDecision::Continue)
            }
            HookAction::RunCommand => {
                let run = self
                    .run_command(rule, "before_tool_call", &event.tool_name, || {
                        serde_json::json!({
                            "event": "before_tool_call",
                            "sessionId": self.session_id,
                            "ruleId": rule.id,
                            "ruleName": rule.name,
                            "toolName": event.tool_name,
                            "callId": event.call_id,
                            "arguments": event.arguments,
                        })
                    })
                    .await;
                let detail = run.detail.as_deref();
                let call_id = Some(event.call_id.as_str());

                match run.verdict {
                    // A tool-call hook has no context channel upstream, so any
                    // text the command printed is dropped rather than smuggled
                    // somewhere it does not belong.
                    CommandVerdict::Proceed { .. } => {
                        self.report(rule, &event.tool_name, outcome::RAN, call_id, detail);
                        Ok(HookDecision::Continue)
                    }
                    CommandVerdict::Deny(reason) => {
                        self.report(rule, &event.tool_name, outcome::DENIED, call_id, detail);
                        Ok(HookDecision::Cancel(reason))
                    }
                    // A hook asked for an opinion that never answered is not
                    // consent — the pending call fails closed, reported as the
                    // failure it is rather than a decision it never made.
                    CommandVerdict::Errored(reason) => {
                        self.report(rule, &event.tool_name, outcome::FAILED, call_id, detail);
                        Ok(HookDecision::Cancel(reason))
                    }
                    CommandVerdict::ReplaceInput(args) => {
                        self.report(rule, &event.tool_name, outcome::REWROTE, call_id, detail);
                        Ok(HookDecision::Replace(args))
                    }
                }
            }
        }
    }

    async fn on_turn_end(
        &self,
        _cx: &ExtensionContext,
        event: &TurnEndEvent,
    ) -> Result<HookDecision, ExtensionError> {
        // A turn has no tool to glob; the needle is tested against the
        // assistant's final text, so a rule can react to WHAT was said
        // ("TODO", "I could not") rather than merely that the turn ended.
        let Some(rule) = self
            .turn_end_rules
            .iter()
            .find(|rule| rule.matches_subject(&MatchSubject::Text(&event.last_assistant_message)))
        else {
            return Ok(HookDecision::Continue);
        };

        // Like the prompt hook's "prompt": the subject named in the notice.
        const SUBJECT: &str = "turn";

        match rule.action {
            HookAction::Notify => {
                self.report(rule, SUBJECT, outcome::OBSERVED, None, None);
                Ok(HookDecision::Continue)
            }
            HookAction::RunCommand => {
                let run = self
                    .run_command(rule, "turn_end", SUBJECT, || {
                        serde_json::json!({
                            "event": "turn_end",
                            "sessionId": self.session_id,
                            "ruleId": rule.id,
                            "ruleName": rule.name,
                            "lastAssistantMessage": event.last_assistant_message,
                            "stopReason": event.stop_reason,
                        })
                    })
                    .await;
                let detail = run.detail.as_deref();

                match run.verdict {
                    // Upstream turns a Cancel here into "the agent does NOT
                    // stop": the reason becomes the model's next instruction,
                    // bounded by the host's re-entry cap. This is the "you
                    // said you'd run the tests — run them" enforcement.
                    CommandVerdict::Deny(reason) => {
                        self.report(rule, SUBJECT, outcome::RESUMED, None, detail);
                        Ok(HookDecision::Cancel(reason))
                    }
                    // A hook that broke must NOT hand the model "hook command
                    // timed out" as an instruction and burn a re-entry on it —
                    // report-only, unlike the pending-call events.
                    CommandVerdict::Errored(_) => {
                        self.report(rule, SUBJECT, outcome::FAILED, None, detail);
                        Ok(HookDecision::Continue)
                    }
                    // There is no pending action to rewrite and no context
                    // channel; the command ran for its side effect.
                    CommandVerdict::Proceed { .. } | CommandVerdict::ReplaceInput(_) => {
                        self.report(rule, SUBJECT, outcome::RAN, None, detail);
                        Ok(HookDecision::Continue)
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
                self.report(
                    rule,
                    &event.tool_name,
                    outcome::OBSERVED,
                    Some(&event.call_id),
                    None,
                );
                Ok(ResultDecision::Continue)
            }
            HookAction::RunCommand => {
                // The call already ran, so a verdict cannot undo it. The command
                // runs for its side effect — format the file just written, commit,
                // notify — and a failure is reported, not enforced.
                let run = self
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
                let detail = run.detail.as_deref();
                let call_id = Some(event.call_id.as_str());

                match run.verdict {
                    // A `Replace` here rewrites what the model reads AND what
                    // the transcript records, which is what makes redaction
                    // actually redact rather than just hide.
                    CommandVerdict::ReplaceInput(result) => {
                        self.report(rule, &event.tool_name, outcome::REWROTE, call_id, detail);
                        Ok(ResultDecision::Replace(result))
                    }
                    // The call already ran, so a denial — or a hook that broke —
                    // cannot undo it; it is reported and the result stands.
                    CommandVerdict::Deny(_) | CommandVerdict::Errored(_) => {
                        self.report(rule, &event.tool_name, outcome::FAILED, call_id, detail);
                        Ok(ResultDecision::Continue)
                    }
                    CommandVerdict::Proceed { .. } => {
                        self.report(rule, &event.tool_name, outcome::RAN, call_id, detail);
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
        assert_eq!(
            events[0]["callId"], "call-bash",
            "the notice must name the call so the timeline can attach it"
        );
        assert_eq!(events[0]["event"], "before_tool_call");
        let detail = events[0]["detail"].as_str().expect("execution capture");
        assert!(
            detail.contains("$ echo") && detail.contains("[exit 0]"),
            "detail should show the command and its exit status, got: {detail}"
        );
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

    fn turn_end(text: &str) -> TurnEndEvent {
        TurnEndEvent {
            last_assistant_message: text.to_string(),
            stop_reason: "end_turn".to_string(),
        }
    }

    /// A turn-end rule matches the assistant's final TEXT, so it can react to
    /// what was said rather than merely that the turn ended.
    #[tokio::test]
    async fn a_turn_end_rule_matches_the_reply_text() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s30".to_string(),
            vec![HookRule {
                arg_field: None,
                arg_contains: Some("TODO".to_string()),
                ..rule("watch todos", HookEvent::TurnEnd, HookAction::Notify)
            }],
        )
        .with_notifier(Some(notifier));

        let skipped = ext
            .on_turn_end(&cx(), &turn_end("all done, tests pass"))
            .await
            .unwrap();
        assert!(matches!(skipped, HookDecision::Continue));
        assert!(seen.lock().unwrap().is_empty(), "no match, no notice");

        let matched = ext
            .on_turn_end(&cx(), &turn_end("left a TODO for later"))
            .await
            .unwrap();
        assert!(matches!(matched, HookDecision::Continue));

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["outcome"], "observed");
        assert_eq!(events[0]["event"], "turn_end");
        assert!(events[0]["callId"].is_null(), "a turn has no tool call");
    }

    /// The point of the event: a command's deny verdict sends the agent back
    /// to work, with the reason as its next instruction.
    #[tokio::test]
    async fn a_turn_end_command_deny_sends_the_agent_back_to_work() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s31".to_string(),
            vec![HookRule {
                arg_field: None,
                arg_contains: None,
                command: Some(
                    r#"echo '{"decision":"deny","reason":"you said you would run the tests"}'"#
                        .to_string(),
                ),
                ..rule("enforce", HookEvent::TurnEnd, HookAction::RunCommand)
            }],
        )
        .with_notifier(Some(notifier))
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext.on_turn_end(&cx(), &turn_end("done!")).await.unwrap();

        match decision {
            HookDecision::Cancel(reason) => {
                assert_eq!(reason, "you said you would run the tests")
            }
            other => panic!("expected Cancel, got {other:?}"),
        }
        let events = seen.lock().unwrap();
        assert_eq!(events[0]["outcome"], "resumed");
    }

    /// A hook that broke must not hand the model its error message as an
    /// instruction — unlike the pending-call events, turn end reports and
    /// lets the agent stop.
    #[tokio::test]
    async fn a_broken_turn_end_command_reports_and_lets_the_turn_end() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s32".to_string(),
            vec![HookRule {
                arg_field: None,
                arg_contains: None,
                command: Some("sleep 5".to_string()),
                timeout_ms: Some(150),
                ..rule("slow", HookEvent::TurnEnd, HookAction::RunCommand)
            }],
        )
        .with_notifier(Some(notifier))
        .with_working_dir(dir.path().to_path_buf());

        let decision = ext.on_turn_end(&cx(), &turn_end("done")).await.unwrap();

        assert!(
            matches!(decision, HookDecision::Continue),
            "a timed-out hook must not burn a re-entry"
        );
        let events = seen.lock().unwrap();
        assert_eq!(events[0]["outcome"], "failed");
        let detail = events[0]["detail"].as_str().expect("execution capture");
        assert!(detail.contains("timed out"), "got: {detail}");
    }

    /// Host-enforced like `on_user_message`: without the declared capability
    /// the hook is never dispatched.
    #[test]
    fn the_turn_end_capability_tracks_whether_rules_exist() {
        let without = RuleHookExtension::new(
            "s33".to_string(),
            vec![rule("tool", HookEvent::BeforeToolCall, HookAction::Notify)],
        );
        assert!(!without.manifest().capabilities.on_turn_end);

        let with = RuleHookExtension::new(
            "s34".to_string(),
            vec![HookRule {
                arg_field: None,
                arg_contains: None,
                ..rule("turn", HookEvent::TurnEnd, HookAction::Notify)
            }],
        );
        assert!(with.manifest().capabilities.on_turn_end);
    }

    /// Approval rules never join the extension chain; only the wrapped
    /// emitter fires them, so registration can be skipped without losing them.
    #[test]
    fn approval_rules_do_not_count_as_extension_rules() {
        let ext = RuleHookExtension::new(
            "s35".to_string(),
            vec![rule(
                "ping",
                HookEvent::ApprovalRequested,
                HookAction::Notify,
            )],
        );
        assert!(!ext.has_extension_rules());
        assert!(ext.has_approval_rules());
    }

    /// `notify` on the approval pause observes it, carrying the call id so
    /// the timeline can attach the notice to its card.
    #[tokio::test]
    async fn a_notify_approval_rule_observes_the_pause() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s36".to_string(),
            vec![rule(
                "ping",
                HookEvent::ApprovalRequested,
                HookAction::Notify,
            )],
        )
        .with_notifier(Some(notifier));

        ext.on_approval_requested("bash", &json!({"command": "rm -rf /tmp/x"}), "call-1")
            .await;
        ext.on_approval_requested("bash", &json!({"command": "ls"}), "call-2")
            .await;

        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1, "only the matching call notifies");
        assert_eq!(events[0]["outcome"], "observed");
        assert_eq!(events[0]["event"], "approval_requested");
        assert_eq!(events[0]["callId"], "call-1");
    }

    /// The approval event is REPORT-ONLY: a command's deny verdict enforces
    /// nothing — the request stays with the user — and is reported as failed.
    #[tokio::test]
    async fn an_approval_command_verdict_enforces_nothing() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let dir = tempfile::TempDir::new().unwrap();
        let ext = RuleHookExtension::new(
            "s37".to_string(),
            vec![HookRule {
                command: Some(r#"echo '{"decision":"deny","reason":"no"}'"#.to_string()),
                ..rule(
                    "objector",
                    HookEvent::ApprovalRequested,
                    HookAction::RunCommand,
                )
            }],
        )
        .with_notifier(Some(notifier))
        .with_working_dir(dir.path().to_path_buf());

        ext.on_approval_requested("bash", &json!({"command": "rm -rf /"}), "call-1")
            .await;

        let events = seen.lock().unwrap();
        assert_eq!(events[0]["outcome"], "failed");
    }

    /// The wrapper forwards the payload untouched and fires the rules from a
    /// spawned task, so a slow hook can never delay the approval prompt.
    #[tokio::test]
    async fn wrap_approval_emitter_forwards_and_fires_rules() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let rules = Arc::new(
            RuleHookExtension::new(
                "s38".to_string(),
                vec![rule(
                    "ping",
                    HookEvent::ApprovalRequested,
                    HookAction::Notify,
                )],
            )
            .with_notifier(Some(notifier)),
        );

        let forwarded: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let inner_sink = forwarded.clone();
        let inner: ApprovalEmitter = Arc::new(move |payload| {
            inner_sink.lock().unwrap().push(payload);
        });

        let wrapped = wrap_approval_emitter(rules, inner);
        wrapped(json!({
            "sessionId": "s38",
            "callId": "call-9",
            "toolName": "bash",
            "args": {"command": "rm -rf /tmp/x"},
            "requestId": "r1",
        }));

        assert_eq!(
            forwarded.lock().unwrap().len(),
            1,
            "the approval payload must reach the frontend regardless of rules"
        );

        // The rule fires on a spawned task; poll briefly for it.
        for _ in 0..50 {
            if !seen.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 1, "the approval rule must fire");
        assert_eq!(events[0]["callId"], "call-9");
        assert_eq!(events[0]["toolName"], "bash");
    }
}
