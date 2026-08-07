/**
 * Declarative hook rules applied to the agent's tool calls.
 * Mirrors `src-tauri/src/storage/types/hook_rule.rs`.
 */

/** Which point of the tool-call lifecycle a rule fires at. */
export type HookEvent =
  | "before_tool_call"
  | "after_tool_call"
  /** The prompt the user submitted, before it enters the transcript. Matched
   *  against the prompt text, and can be rewritten or refused. */
  | "user_prompt_submit";

/**
 * What a matching rule does. Hooks execute actions rather than gate calls —
 * permission control lives in the agent's own configuration. `notify` observes
 * and reports; `run_command` runs the rule's command, whose output may still
 * contribute context, rewrite arguments/results, or veto the call. Both are
 * valid on every event.
 */
export type HookAction = "notify" | "run_command";

export interface HookRule {
  id: string;
  name: string;
  event: HookEvent;
  /** Tool-name glob: `*` alone, or one leading/trailing `*`. */
  toolPattern: string;
  /** Argument to inspect; null matches against the whole arguments object. */
  argField: string | null;
  /** Substring the argument must contain; null matches on the tool pattern alone. */
  argContains: string | null;
  action: HookAction;
  /** Shown to the user alongside the match notice. */
  message: string | null;
  /** Shell command for `run_command`; ignored by `notify`. */
  command: string | null;
  /** Budget for the command in ms; null uses the backend default (10s). */
  timeoutMs: number | null;
  enabled: boolean;
  /** Evaluation order; the first matching rule decides the call. */
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface CreateHookRuleRequest {
  name: string;
  event: HookEvent;
  toolPattern: string;
  argField?: string | null;
  argContains?: string | null;
  action: HookAction;
  message?: string | null;
  command?: string | null;
  timeoutMs?: number | null;
  sortOrder?: number | null;
}

/**
 * Every field optional; an omitted field keeps its stored value. For the three
 * nullable fields an **empty string clears** them.
 */
export interface UpdateHookRuleRequest {
  name?: string;
  event?: HookEvent;
  toolPattern?: string;
  argField?: string;
  argContains?: string;
  action?: HookAction;
  message?: string;
  command?: string;
  timeoutMs?: number;
  enabled?: boolean;
  sortOrder?: number;
}

/** What actually happened; a command resolves several ways. */
export type HookRuleOutcome =
  /** A command's verdict blocked the call (or the prompt). */
  | "denied"
  /** A `notify` rule matched. */
  | "observed"
  /** A `run_command` hook ran and raised no objection. */
  | "ran"
  /** Its command rewrote the tool's arguments. */
  | "rewrote"
  /** It failed after the call had run, so nothing could be undone. */
  | "failed"
  /** Its command contributed context for the model to read this turn. */
  | "informed";

/**
 * Payload of `agent_hook_rule_notify`, emitted on **every** rule match, not
 * just the `notify` action — otherwise a rule that blocks or waves through a
 * call is indistinguishable from no rule matching at all.
 */
export interface HookRuleNotification {
  sessionId: string;
  ruleId: string;
  ruleName: string;
  action: HookAction;
  /** Which lifecycle point fired — placement in the timeline follows it. */
  event: HookEvent;
  toolName: string;
  /** Tool call this firing belongs to; null for prompt rules. */
  callId: string | null;
  outcome: HookRuleOutcome;
  message: string | null;
  /** A command's execution capture: command line, exit status, output. */
  detail: string | null;
}
