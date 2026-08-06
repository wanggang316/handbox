/**
 * Surfaces hook-rule matches as toasts.
 *
 * A rule that changes what the agent may do without saying so is worse than no
 * rule: the user cannot tell "no rule matched" from "a rule fired and I missed
 * it". The backend reports every match on `agent_hook_rule_notify`; this turns
 * each one into a visible notice.
 */
import { listenToAgentStreamEvents } from "$lib/api/agentSession";
import type { HookRuleNotification } from "$lib/types";
import { toastActions } from "./toast.svelte";
import { t } from "$lib/i18n";

class HookRuleNoticeStore {
  private unlisten: (() => void) | null = null;

  constructor() {
    // Global and once, mirroring the approval listener: matches arrive for
    // whichever session is running.
    void this.initListener();
  }

  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onHookRuleMatch: (payload) => this.notify(payload),
      });
    } catch (error) {
      console.error("Failed to init hook rule listener:", error);
    }
  }

  /**
   * A denial is the one outcome that changed the result against the agent's
   * intent, so it warns; the rest are informational.
   */
  private notify(payload: HookRuleNotification): void {
    const text = t(`settings.hooks.notice.${payload.outcome}`)
      .replace("{rule}", payload.ruleName)
      .replace("{tool}", payload.toolName);

    const detail = payload.message ? `${text} — ${payload.message}` : text;

    if (payload.outcome === "denied" || payload.outcome === "rejected") {
      toastActions.warning(detail);
    } else {
      toastActions.info(detail);
    }
  }
}

export const hookRuleNoticeStore = new HookRuleNoticeStore();
