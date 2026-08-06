/**
 * Declarative hook rules applied to the agent's tool calls.
 * Mirrors `src-tauri/src/storage/types/hook_rule.rs`.
 */

/** Which point of the tool-call lifecycle a rule fires at. */
export type HookEvent = "before_tool_call" | "after_tool_call";

/**
 * What a matching rule does. `deny` / `ask` / `allow` decide a pending call and
 * are only valid on `before_tool_call`; `notify` observes a finished one and is
 * only valid on `after_tool_call`. The backend rejects the other combinations.
 */
export type HookAction = "deny" | "ask" | "allow" | "notify";

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
  /** Shown to the model on `deny`, and to the user on `ask` / `notify`. */
  message: string | null;
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
  enabled?: boolean;
  sortOrder?: number;
}

/** What actually happened to the call; an `ask` resolves either way. */
export type HookRuleOutcome =
  | "denied"
  | "allowed"
  | "approved"
  | "rejected"
  | "observed";

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
  toolName: string;
  outcome: HookRuleOutcome;
  message: string | null;
}
