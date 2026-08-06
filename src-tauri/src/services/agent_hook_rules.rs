//! Evaluates the user's declarative hook rules against the agent's tool calls.
//!
//! Registered BETWEEN [`SandboxExtension`](super::agent_permission::SandboxExtension)
//! and [`PermissionExtension`](super::agent_permission::PermissionExtension):
//! after the sandbox so a rule can never widen the working-directory boundary,
//! before the approval gate so an `allow` rule can spare the user a prompt they
//! have already answered in the form of a rule.
//!
//! Rules are a snapshot taken when the session is built. A session is
//! constructed per turn, so an edit in settings takes effect on the next
//! message rather than mid-turn — which also means a rule cannot change under a
//! tool call that is already being judged.

use async_trait::async_trait;
use std::sync::Arc;

use hand_coding_agent::core::extensions::api::{ToolCallEvent, ToolResultEvent};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};

use crate::services::agent_permission::{clear_call_for_rule, request_approval, ApprovalEmitter};
use crate::storage::types::{HookAction, HookEvent, HookRule};

const RULE_EXTENSION_NAME: &str = "handbox-hook-rules";

/// Tauri event emitted whenever a rule matches — not only for the `notify`
/// action. A rule that silently changes what the agent may do is worse than no
/// rule: the user cannot tell "no rule matched" from "a rule fired and I missed
/// it". Carries `{ sessionId, ruleId, ruleName, action, toolName, outcome,
/// message }`.
pub const HOOK_RULE_NOTIFY_EVENT: &str = "agent_hook_rule_notify";

/// What actually happened to the call, for the notification payload. Distinct
/// from the rule's action because an `ask` resolves either way.
mod outcome {
    pub const DENIED: &str = "denied";
    pub const ALLOWED: &str = "allowed";
    pub const APPROVED: &str = "approved";
    pub const REJECTED: &str = "rejected";
    pub const OBSERVED: &str = "observed";
}

/// Sink for [`HOOK_RULE_NOTIFY_EVENT`]. Same shape as [`ApprovalEmitter`] so the
/// extension stays free of Tauri types and is testable with a plain closure.
pub type NotifyEmitter = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// Applies the user's rules to each tool call.
pub struct RuleHookExtension {
    manifest: ExtensionManifest,
    /// HandBox DB session id (UUID) — the same key the approval registry and
    /// `abort_run` use. See [`PermissionExtension`](super::agent_permission::PermissionExtension).
    session_id: String,
    /// `None` makes an `ask` rule fail closed, exactly like the approval gate.
    emitter: Option<ApprovalEmitter>,
    notifier: Option<NotifyEmitter>,
    /// Pre-split by event so neither hook filters at call time. Both keep the
    /// repository's `sort_order` — first match decides.
    before_rules: Vec<HookRule>,
    after_rules: Vec<HookRule>,
}

impl RuleHookExtension {
    pub fn new(session_id: String, rules: Vec<HookRule>) -> Self {
        let (before_rules, after_rules) = rules
            .into_iter()
            .partition(|rule| rule.event == HookEvent::BeforeToolCall);

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
            emitter: None,
            notifier: None,
            before_rules,
            after_rules,
        }
    }

    /// Wire the approval surface an `ask` rule prompts through. Without it such
    /// a rule denies rather than silently allowing.
    pub fn with_approval_emitter(mut self, emitter: Option<ApprovalEmitter>) -> Self {
        self.emitter = emitter;
        self
    }

    /// Wire the sink for `notify` rules. Without it a `notify` rule is inert.
    pub fn with_notifier(mut self, notifier: Option<NotifyEmitter>) -> Self {
        self.notifier = notifier;
        self
    }

    /// Whether any rule is loaded — lets the caller skip registering an
    /// extension that would do nothing on every tool call.
    pub fn is_empty(&self) -> bool {
        self.before_rules.is_empty() && self.after_rules.is_empty()
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

/// The refusal handed to the model. Uses the rule's own message when set, so the
/// model can relay *why* rather than a generic block, and always names the rule
/// so the user can find which one fired.
fn rule_deny_reason(rule: &HookRule, tool_name: &str) -> String {
    match rule.message.as_deref().filter(|m| !m.is_empty()) {
        Some(message) => format!("{message}（hook rule: {}）", rule.name),
        None => format!("{tool_name} 被 hook 规则「{}」拦截（denied）", rule.name),
    }
}

#[async_trait]
impl Extension for RuleHookExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
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
            HookAction::Deny => {
                self.report(rule, &event.tool_name, outcome::DENIED);
                Ok(HookDecision::Cancel(rule_deny_reason(
                    rule,
                    &event.tool_name,
                )))
            }
            HookAction::Allow => {
                // Consent for this one call, so the approval gate behind us does
                // not prompt for what a rule already permits.
                //
                // Only when a consent surface exists at all. A headless run (a
                // scheduled job) has no emitter and its gate fails closed by
                // design; letting a rule clear that would widen what an
                // unattended run may do, which is not what writing a rule in the
                // settings UI asks for.
                if self.emitter.is_some() {
                    clear_call_for_rule(&event.call_id);
                }
                self.report(rule, &event.tool_name, outcome::ALLOWED);
                Ok(HookDecision::Continue)
            }
            HookAction::Ask => {
                let decision = request_approval(
                    &self.session_id,
                    self.emitter.as_ref(),
                    &event.call_id,
                    &event.tool_name,
                    &event.arguments,
                )
                .await;
                // The user just answered for this call; clear it so a dangerous
                // tool does not raise a second, identical dialog.
                let approved = matches!(decision, HookDecision::Continue);
                if approved {
                    clear_call_for_rule(&event.call_id);
                }
                self.report(
                    rule,
                    &event.tool_name,
                    if approved {
                        outcome::APPROVED
                    } else {
                        outcome::REJECTED
                    },
                );
                Ok(decision)
            }
            // Only meaningful after a call; a rule stored this way is inert
            // rather than an error, so a mis-set action never blocks work.
            HookAction::Notify => Ok(HookDecision::Continue),
        }
    }

    async fn on_after_tool_call(
        &self,
        _cx: &ExtensionContext,
        event: &ToolResultEvent,
    ) -> Result<(), ExtensionError> {
        // After a call there are no arguments left to match on, so the needle is
        // tested against the tool's RESULT.
        let Some(rule) = self
            .after_rules
            .iter()
            .find(|rule| rule.matches(&event.tool_name, &event.result))
        else {
            return Ok(());
        };

        if rule.action != HookAction::Notify {
            // A decision action cannot un-run a finished call.
            return Ok(());
        }

        self.report(rule, &event.tool_name, outcome::OBSERVED);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_permission::{respond_to_approval, ApprovalDecision};
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
            vec![rule(
                "block rm",
                HookEvent::BeforeToolCall,
                HookAction::Deny,
            )],
        );
        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "ls"})))
            .await
            .unwrap();
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn a_deny_rule_cancels_with_its_own_message() {
        let mut r = rule("block rm", HookEvent::BeforeToolCall, HookAction::Deny);
        r.message = Some("危险命令".to_string());
        let ext = RuleHookExtension::new("s1".to_string(), vec![r]);

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();
        match decision {
            HookDecision::Cancel(reason) => {
                assert!(
                    reason.contains("危险命令"),
                    "rule message reaches the model"
                );
                assert!(reason.contains("block rm"), "and names the rule that fired");
            }
            other => panic!("expected Cancel, got {other:?}"),
        }
    }

    /// The first rule in `sort_order` decides; a later rule matching the same
    /// call never runs.
    #[tokio::test]
    async fn the_first_matching_rule_wins() {
        let allow = HookRule {
            sort_order: 0,
            ..rule("allow first", HookEvent::BeforeToolCall, HookAction::Allow)
        };
        let deny = HookRule {
            sort_order: 1,
            ..rule("deny second", HookEvent::BeforeToolCall, HookAction::Deny)
        };
        let ext = RuleHookExtension::new("s1".to_string(), vec![allow, deny]);

        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /tmp/x"})))
            .await
            .unwrap();
        assert!(
            matches!(decision, HookDecision::Continue),
            "the allow rule sorted first decides the call"
        );
    }

    /// An `allow` rule clears the call so the approval gate registered behind
    /// this extension does not prompt for it.
    #[tokio::test]
    async fn an_allow_rule_clears_the_call_for_the_approval_gate() {
        let ext = RuleHookExtension::new(
            "s1".to_string(),
            vec![rule("auto", HookEvent::BeforeToolCall, HookAction::Allow)],
        )
        .with_approval_emitter(Some(Arc::new(|_| {})));
        let event = call("bash", json!({"command": "rm -rf /tmp/x"}));
        ext.on_before_tool_call(&cx(), &event).await.unwrap();

        // The permission extension consumes the clearance; asserting through its
        // public hook keeps this test honest about the real interaction.
        let permission = crate::services::agent_permission::PermissionExtension::new(
            "s1".to_string(),
            // No emitter: without a clearance this would fail closed and Cancel.
            None,
        );
        let decision = permission.on_before_tool_call(&cx(), &event).await.unwrap();
        assert!(
            matches!(decision, HookDecision::Continue),
            "the cleared call skips the approval gate instead of failing closed"
        );
    }

    /// A clearance is per call id and consumed once — it must not become
    /// standing consent for the tool.
    #[tokio::test]
    async fn a_clearance_does_not_carry_to_the_next_call() {
        let ext = RuleHookExtension::new(
            "s2".to_string(),
            vec![HookRule {
                arg_contains: Some("git ".to_string()),
                ..rule("allow git", HookEvent::BeforeToolCall, HookAction::Allow)
            }],
        )
        .with_approval_emitter(Some(Arc::new(|_| {})));
        let allowed = ToolCallEvent {
            call_id: "call-a".to_string(),
            ..call("bash", json!({"command": "git status"}))
        };
        ext.on_before_tool_call(&cx(), &allowed).await.unwrap();

        let dangerous = ToolCallEvent {
            call_id: "call-b".to_string(),
            ..call("bash", json!({"command": "rm -rf /"}))
        };
        let permission =
            crate::services::agent_permission::PermissionExtension::new("s2".to_string(), None);
        let decision = permission
            .on_before_tool_call(&cx(), &dangerous)
            .await
            .unwrap();
        assert!(
            matches!(decision, HookDecision::Cancel(_)),
            "a different call must still be gated"
        );
    }

    /// Headless runs (scheduled jobs) have no consent surface, and their approval
    /// gate fails closed by design. An `allow` rule must not quietly widen that:
    /// with no emitter the clearance is never granted.
    #[tokio::test]
    async fn an_allow_rule_does_not_widen_a_headless_run() {
        let ext = RuleHookExtension::new(
            "s7".to_string(),
            vec![rule("auto", HookEvent::BeforeToolCall, HookAction::Allow)],
        );
        let event = call("bash", json!({"command": "rm -rf /tmp/x"}));

        let decision = ext.on_before_tool_call(&cx(), &event).await.unwrap();
        assert!(
            matches!(decision, HookDecision::Continue),
            "the rule itself still passes the call along"
        );

        let permission =
            crate::services::agent_permission::PermissionExtension::new("s7".to_string(), None);
        let gated = permission.on_before_tool_call(&cx(), &event).await.unwrap();
        assert!(
            matches!(gated, HookDecision::Cancel(_)),
            "but the gate still fails closed without a consent surface"
        );
    }

    /// With no approval surface an `ask` rule denies rather than allowing.
    #[tokio::test]
    async fn an_ask_rule_without_an_emitter_fails_closed() {
        let ext = RuleHookExtension::new(
            "s3".to_string(),
            vec![rule("confirm", HookEvent::BeforeToolCall, HookAction::Ask)],
        );
        let decision = ext
            .on_before_tool_call(&cx(), &call("bash", json!({"command": "rm -rf /"})))
            .await
            .unwrap();
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    /// An `ask` rule prompts through the shared approval registry, so the
    /// existing `agent_approval_respond` IPC answers it unchanged.
    #[tokio::test]
    async fn an_ask_rule_prompts_and_honors_the_answer() {
        let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let sink = captured.clone();
        let emitter: ApprovalEmitter = Arc::new(move |payload: serde_json::Value| {
            *sink.lock().unwrap() = payload["requestId"].as_str().map(str::to_string);
        });

        let ext = RuleHookExtension::new(
            "s4".to_string(),
            vec![rule("confirm", HookEvent::BeforeToolCall, HookAction::Ask)],
        )
        .with_approval_emitter(Some(emitter));

        let event = call("bash", json!({"command": "rm -rf /tmp/x"}));
        let hook = tokio::spawn(async move { ext.on_before_tool_call(&cx(), &event).await });

        // Wait for the request to be registered, then answer it.
        let request_id = loop {
            if let Some(id) = captured.lock().unwrap().clone() {
                break id;
            }
            tokio::task::yield_now().await;
        };
        respond_to_approval(&request_id, ApprovalDecision::AllowOnce);

        let decision = hook.await.unwrap().unwrap();
        assert!(matches!(decision, HookDecision::Continue));
    }

    /// A block must announce itself. Without this the user cannot tell a rule
    /// firing apart from no rule matching — the failure that made the feature
    /// look broken in real use.
    #[tokio::test]
    async fn a_deny_rule_reports_the_block() {
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let notifier: NotifyEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));

        let ext = RuleHookExtension::new(
            "s8".to_string(),
            vec![rule(
                "block rm",
                HookEvent::BeforeToolCall,
                HookAction::Deny,
            )],
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
                "block rm",
                HookEvent::BeforeToolCall,
                HookAction::Deny,
            )],
        )
        .with_notifier(Some(notifier));

        ext.on_before_tool_call(&cx(), &call("bash", json!({"command": "ls"})))
            .await
            .unwrap();

        assert!(seen.lock().unwrap().is_empty());
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
            vec![rule(
                "after only",
                HookEvent::AfterToolCall,
                HookAction::Deny,
            )],
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
