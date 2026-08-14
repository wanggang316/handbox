/**
 * Agent question state (`ask_question` tool) - Svelte 5 runes.
 *
 * When the model calls `ask_question`, the backend tool emits
 * `agent_question_request` and parks the turn awaiting the user's answer; this
 * store holds pending requests keyed by sessionId, so the question panel slides
 * up over that session's composer and the conversation pauses. Answers flow back
 * via `respondTo(request, response)`: `answered` hands the values to the model as
 * the tool result, `dismissed` tells it the user wants to keep talking instead.
 *
 * Design mirrors `agentApproval.svelte.ts`:
 *  - a one-time navigation-resilient listener is established at singleton
 *    construction (never unmounts on route / mode switches);
 *  - the listener subscribes only to the `agent_question_request` channel,
 *    independent of the run channels and the chat stream;
 *  - write paths (event arrival / answer) update a reactive map, read paths use
 *    reference-stable getters.
 *
 * Independent of run state: questions never enter the run reducer and do not
 * affect the closed-once contract. Unlike an approval, a question is NOT a
 * security gate — dismissing it is a first-class outcome, not a denial.
 */

import type {
  AgentQuestionRequest,
  AgentQuestionResponse,
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  respondAgentQuestion,
} from "$lib/api/agentSession";

class AgentQuestionStore {
  // Pending requests keyed by sessionId, at most one in-flight per session: the
  // agent loop awaits the parked tool call before issuing another, so a single
  // value expresses "this session is paused on this question set". A new
  // request overwriting the old key is a defensive fallback.
  private pending = $state<Record<string, AgentQuestionRequest>>({});

  // Cleanup for the one-time stream listener (rarely called in the store's lifetime).
  private unlisten: (() => void) | null = null;

  constructor() {
    // Listener established at singleton construction: navigation-resilient,
    // keeps receiving across route and mode switches.
    void this.initListener();
  }

  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onQuestionRequest: (payload) => this.handleQuestionRequest(payload),
        onClosed: (payload) => this.handleRunClosed(payload.sessionId),
      });
    } catch (error) {
      console.error("Failed to init agent question listener:", error);
    }
  }

  /**
   * Dispatch `agent_question_request`: record the request by sessionId so the
   * panel opens and that session's composer pauses.
   */
  private handleQuestionRequest(payload: AgentQuestionRequest): void {
    this.pending = { ...this.pending, [payload.sessionId]: payload };
  }

  /**
   * The run ended (normally or via abort) — drop that session's panel.
   *
   * Required for the abort path: `abort_run` cancels the parked tool call
   * backend-side but emits no question-channel event, so without this the panel
   * would linger over a finished run and keep the composer disabled. Safe
   * unconditionally: a turn cannot close while still parked on a question, so a
   * request pending at close time is always dead.
   */
  private handleRunClosed(sessionId: string): void {
    if (!this.pending[sessionId]) {
      return;
    }
    const next = { ...this.pending };
    delete next[sessionId];
    this.pending = next;
  }

  /**
   * Reactive getter: the session's currently pending request (`null` if none).
   * AgentInput mounts the panel on it and pauses the composer. READ-ONLY: never
   * write `$state` here (the getter is consumed by `$derived` / templates;
   * writing would throw Svelte's state_unsafe_mutation).
   */
  pendingFor(sessionId: string): AgentQuestionRequest | null {
    return this.pending[sessionId] ?? null;
  }

  hasPending(sessionId: string): boolean {
    return !!this.pending[sessionId];
  }

  /**
   * Answer THE request the panel is showing: the response targets
   * `request.requestId`, so the displayed request == the answered target, never
   * mis-hitting a newer request that overwrote the key (same structural race the
   * approval store avoids by taking the dialog's own reference).
   *
   * Clear the key before responding: instant UI feedback; a failed response is
   * only logged and the clear is not rolled back — the backend is an idempotent
   * no-op for unknown / duplicate `requestId`s, and re-showing the panel would
   * make the user re-answer a request the backend may have already abandoned.
   */
  async respondTo(
    request: AgentQuestionRequest,
    response: AgentQuestionResponse,
  ): Promise<void> {
    this.clearRequest(request);
    try {
      await respondAgentQuestion(request.requestId, response);
    } catch (error) {
      console.error("Failed to respond to agent question:", error);
    }
  }

  /**
   * Clear a specific request (close the panel / unpause). Clears ONLY when the
   * sessionId's current key holds exactly this request (compared by requestId):
   * if a newer request for the same session overwrote the key during the user's
   * answering window, clearing would wrongly dismiss an unanswered request.
   * Write-path only, so no reactive-read hazard.
   */
  private clearRequest(request: AgentQuestionRequest): void {
    if (this.pending[request.sessionId]?.requestId !== request.requestId) {
      return;
    }
    const next = { ...this.pending };
    delete next[request.sessionId];
    this.pending = next;
  }
}

// Singleton; construction sets up the navigation-resilient listener.
export const agentQuestionStore = new AgentQuestionStore();
