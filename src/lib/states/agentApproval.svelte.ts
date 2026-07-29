/**
 * Agent tool-approval state - Svelte 5 runes.
 *
 * When a dangerous tool (write/edit/bash) is invoked, the backend
 * `PermissionExtension` emits `agent_approval_request` and awaits the user's
 * decision; this store holds pending requests keyed by sessionId, so the
 * approval dialog opens and that session's conversation pauses. Decisions flow
 * back via `respondAgentApproval(requestId, allow)`: allow → the tool executes
 * and the conversation continues; deny → the tool is cancelled, the model
 * receives the denied result, and the conversation continues uninterrupted.
 *
 * Design mirrors `agentRun.svelte.ts`:
 *  - a one-time navigation-resilient listener is established at singleton
 *    construction (never unmounts on route / mode switches);
 *  - the listener subscribes only to the `agent_approval_request` channel,
 *    independent of the run channels and the chat stream;
 *  - write paths (event arrival / decision) update a reactive map, read paths
 *    use reference-stable getters.
 *
 * Independent of run state: approvals never enter the run reducer and do not
 * affect the closed-once contract; run state reflects tool execution progress,
 * approval state only reflects "is a dangerous call awaiting a decision".
 * AgentInput / pages use `pendingFor(sessionId)` to pause input and mount the
 * dialog.
 *
 * Scope flows back via the three-way `decision`: `allow_once` is one-shot,
 * `allow_always` remembers the tool for this session (backend in-process
 * memory set, not across sessions/restarts), `deny` rejects.
 */

import type {
  AgentApprovalRequest,
  ApprovalDecision,
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  respondAgentApproval,
} from "$lib/api/agentSession";

class AgentApprovalStore {
  // Pending requests keyed by sessionId, at most one in-flight per session:
  // while awaiting a decision, the backend never issues a second concurrent
  // dangerous call for the same session's run (the hook chain awaits
  // serially), so a single value expresses "this session is paused on this
  // request". A new request overwriting the old key is a defensive fallback.
  private pending = $state<Record<string, AgentApprovalRequest>>({});

  // Cleanup for the one-time stream listener (rarely called in the store's lifetime).
  private unlisten: (() => void) | null = null;

  constructor() {
    // Listener established at singleton construction: navigation-resilient,
    // keeps receiving across route and mode switches.
    void this.initListener();
  }

  /**
   * Set up the global agent approval listener (once). Requests are stored
   * keyed by the payload's sessionId.
   */
  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onApprovalRequest: (payload) => this.handleApprovalRequest(payload),
      });
    } catch (error) {
      console.error("Failed to init agent approval listener:", error);
    }
  }

  /**
   * Dispatch `agent_approval_request`: record the request by sessionId so the
   * dialog opens and that session's conversation pauses. The displayed `args`
   * are the exact arguments about to execute.
   */
  private handleApprovalRequest(payload: AgentApprovalRequest): void {
    this.pending = { ...this.pending, [payload.sessionId]: payload };
  }

  /**
   * Reactive getter: the session's currently pending request (`null` if none).
   * AgentInput pauses input when non-null; pages mount the approval dialog on
   * it. READ-ONLY: never write `$state` here (the getter is consumed by
   * `$derived` / templates; writing would throw Svelte's
   * state_unsafe_mutation).
   */
  pendingFor(sessionId: string): AgentApprovalRequest | null {
    return this.pending[sessionId] ?? null;
  }

  hasPending(sessionId: string): boolean {
    return !!this.pending[sessionId];
  }

  /**
   * Respond to THE request the dialog is showing (scope included): the
   * response targets `request.requestId`, so the displayed request == the
   * responded target, never mis-hitting a newer request that overwrote the
   * key.
   *
   * Takes the dialog's own request reference (rather than re-querying the
   * store by sessionId): this store keeps one value per sessionId, so if a new
   * request for the same session overwrites the key during the user's
   * decision window, a sessionId lookup would read the new requestId — the
   * dialog still shows the old request but the decision would land on the new
   * one. Using the dialog's reference eliminates this structural race; the
   * backend's exact requestId routing + first-wins remains the fallback, but
   * the frontend must not pick the wrong target in the first place.
   *
   * `decision` is three-way: `allow_once` allows this call only (no memory),
   * `allow_always` always allows this tool for this session (backend
   * in-process set keyed by sessionId; no further dialogs for the same
   * session+tool), `deny` rejects. The `allow_always` scope memory lives in
   * the backend; the frontend just passes the decision through.
   *
   * Clear the key before responding: instant UI feedback; a failed response is
   * only logged and the clear is not rolled back — the backend is an
   * idempotent no-op for unknown / duplicate `requestId`s, and re-showing the
   * dialog would make the user re-decide a request the backend may have
   * already abandoned.
   */
  async respondTo(
    request: AgentApprovalRequest,
    decision: ApprovalDecision,
  ): Promise<void> {
    this.clearRequest(request);
    try {
      await respondAgentApproval(request.requestId, decision);
    } catch (error) {
      console.error("Failed to respond to agent approval:", error);
    }
  }

  /**
   * Clear a specific request (close the dialog / unpause). Clears ONLY when
   * the sessionId's current key holds exactly this request (compared by
   * requestId): if a newer request for the same session overwrote the key
   * during the decision window, clearing would wrongly dismiss an undecided
   * request — the equality guard prevents that. Write-path only, so no
   * reactive-read hazard.
   */
  private clearRequest(request: AgentApprovalRequest): void {
    if (this.pending[request.sessionId]?.requestId !== request.requestId) {
      return;
    }
    const next = { ...this.pending };
    delete next[request.sessionId];
    this.pending = next;
  }
}

// Singleton; construction sets up the navigation-resilient listener.
export const agentApprovalStore = new AgentApprovalStore();
