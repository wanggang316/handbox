/**
 * Agent run state - Svelte 5 runes.
 *
 * Reducer/listener contract, keyed by sessionId: each session owns its own
 * committed transcript and streaming view-model, so a session streaming in the
 * background keeps updating while a different session is in the foreground.
 *
 * The stream listener is established once at store-singleton construction
 * (navigation-resilient): the singleton lives for the whole app lifetime, the
 * listener never unmounts on route changes, and it only subscribes to
 * `agent_stream_*` events.
 *
 * Tool calls: `tool_execution_start/update/end` events reduce into a live
 * tool-call view-model keyed by `toolCallId`, rendered by the timeline as tool
 * cards. Committed `toolcall` content blocks + `toolResult` messages (restored
 * from persistence) reconcile with live state: one `toolCallId` yields exactly
 * one card, whether live or restored.
 */

import type { UUID } from "$lib/types";
import type { HookRuleNotification } from "$lib/types";
import type {
  AgentMessage,
  AgentSessionMessage,
  AgentEvent,
  AssistantMessageEvent,
  AgentStreamEventPayload,
  AgentStreamErrorPayload,
  AgentStreamClosedPayload,
  AgentSessionLifecyclePayload,
  ToolResultContent,
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  getAgentSessionMessages,
  abortAgentRun,
} from "$lib/api/agentSession";
import { agentSessionActions } from "$lib/states/agentSession.svelte";

/**
 * Normalized tool-call view-model (the single shape tool cards consume).
 *
 * The live path (during a run) is reduced from `tool_execution_*` events; the
 * restored path (after reload) is normalized from committed `toolcall` content
 * blocks + paired `toolResult` messages. Both map to this shape so
 * `AgentToolCallCard` renders the same card regardless of source.
 *
 * `status`: `executing` (started, result pending) / `completed` (ended, not an
 * error) / `error` (ended with isError, or restored `toolResult.isError`).
 * `result` holds the final tool-result content blocks (text / image);
 * undefined while `executing`.
 */
export type ToolCallStatus = "executing" | "completed" | "error";

export interface ToolCallView {
  toolCallId: string;
  toolName: string;
  args: unknown;
  status: ToolCallStatus;
  result?: ToolResultContent[];
}

/**
 * One hook-rule firing, anchored into the timeline. `anchor` is the index of
 * the last committed message at arrival time (-1 before any message), so the
 * timeline renders the entry right after the message it chronologically
 * followed — a hook on a tool call lands under the assistant turn that issued
 * the call. Ephemeral view state: not persisted, cleared on transcript restore.
 */
export interface HookNoticeEntry {
  anchor: number;
  notice: HookRuleNotification;
}

/**
 * Per-session run view-model.
 *
 * `messages` is the committed (finalized) message sequence; `streamingText` /
 * `thinkingText` accumulate the currently streaming assistant text/thinking;
 * `isRunning` marks an active run; `error` is the session's latest run-level
 * error (arrives before `agent_stream_closed`). `toolCalls` is the live
 * tool-call view-model keyed by `toolCallId`: start/update/end for the same
 * call land on the same entry, so the card flips in place from executing to
 * its final state.
 */
export interface AgentRunState {
  messages: AgentMessage[];
  streamingText: string;
  thinkingText: string;
  isRunning: boolean;
  error: string | null;
  toolCalls: Record<string, ToolCallView>;
  /**
   * Auto-compaction in progress (long sessions trigger context compaction).
   * Set true on `compaction_start`, false on `compaction_end`; the timeline
   * shows a distinct "compacting" indicator. Compaction happens within a turn
   * and emits no extra closed event, so this flag is independent of
   * `isRunning`: the turn resumes afterwards and still closes exactly once.
   */
  isCompacting: boolean;
  /**
   * Hook-rule firings for this session, in arrival order. Rendered inline in
   * the timeline (a transient toast proved too easy to miss).
   */
  hookNotices: HookNoticeEntry[];
  /**
   * Whether this session's committed transcript has been restored at least
   * once (`loadTranscript`). The session page uses it to tell "not yet loaded
   * (centered spinner)" from "genuinely empty session (onboarding empty
   * state)", avoiding a one-frame empty-state flash when first opening a
   * session with history. Persists per sessionId in this store singleton, so
   * revisits render from cache without returning to the spinner.
   */
  hydrated: boolean;
}

function createEmptyRunState(): AgentRunState {
  return {
    messages: [],
    streamingText: "",
    thinkingText: "",
    isRunning: false,
    error: null,
    toolCalls: {},
    isCompacting: false,
    hookNotices: [],
    hydrated: false,
  };
}

// Shared read-only empty state: lets `runStateFor` return a reference-stable
// placeholder before a session's state exists, so `$derived` / template reads
// never write `$state` (Svelte 5 state_unsafe_mutation). Read-only; real
// per-session state is lazily created by write paths (loadTranscript / event reduce).
const EMPTY_RUN_STATE: AgentRunState = Object.freeze(createEmptyRunState());

class AgentRunStore {
  // Run state keyed by sessionId; sessions are independent.
  private states = $state<Record<string, AgentRunState>>({});

  // Tombstones for deleted sessions: after `removeSession`, late stream events
  // from an in-flight run (tool_execution_end / agent_stream_closed emitted by
  // the pre-delete abort) must not recreate the deleted entry via `ensureState`.
  // The guard only applies to explicitly deleted ids; normal streaming is
  // unaffected. `agent_stream_closed` is the run's terminal signal and reclaims
  // the tombstone on arrival (no further events for that run; a deleted session
  // gets no new runs). A tombstone with no in-flight run at delete time is
  // never reclaimed and simply persists — entries are UUID strings, negligible
  // in size, and session ids are not reused, so nothing is mis-intercepted.
  // Non-reactive internal bookkeeping, not `$state`.
  private deletedSessions = new Set<string>();

  // Cleanup for the one-time stream listener (rarely called in the store's lifetime).
  private unlisten: (() => void) | null = null;

  // Run-termination callback, fired when `agent_stream_closed` arrives.
  // Registered by the sidebar state layer to refresh that session's metadata /
  // ordering without this store depending on session state (one-way:
  // agentSession does not import agentRun).
  private onRunClosed: ((sessionId: string) => void) | null = null;

  // Session-name-change callback, fired on the `session_info_changed`
  // lifecycle signal. Registered by the sidebar state layer to update the
  // session title immediately; same one-way wiring as `onRunClosed`.
  private onSessionInfoChanged:
    | ((sessionId: string, name: string | null) => void)
    | null = null;

  constructor() {
    // Listener established at singleton construction: navigation-resilient,
    // keeps reducing across route and mode switches.
    void this.initListener();
  }

  /**
   * Set up the global agent stream listener (once). Every event is dispatched
   * by its payload's sessionId.
   */
  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onEvent: (payload) => this.handleStreamEvent(payload),
        onError: (payload) => this.handleStreamError(payload),
        onClosed: (payload) => this.handleStreamClosed(payload),
        onLifecycle: (payload) => this.handleLifecycle(payload),
        onHookRuleMatch: (payload) => this.addHookNotice(payload),
      });
    } catch (error) {
      console.error("Failed to init agent stream listener:", error);
    }
  }

  /** Get (lazily creating) a session's mutable run state. */
  private ensureState(sessionId: string): AgentRunState {
    if (!this.states[sessionId]) {
      this.states[sessionId] = createEmptyRunState();
    }
    return this.states[sessionId];
  }

  /**
   * A hook rule fired: anchor it after the message committed most recently, so
   * the timeline shows it where it happened (under the assistant turn whose
   * tool call it ran on). Reported on every match — a hook that silently acts
   * is indistinguishable from one that never fired.
   */
  private addHookNotice(notice: HookRuleNotification): void {
    if (this.deletedSessions.has(notice.sessionId)) {
      return;
    }
    const state = this.ensureState(notice.sessionId);
    state.hookNotices = [
      ...state.hookNotices,
      { anchor: state.messages.length - 1, notice },
    ];
  }

  /**
   * Dispatch `agent_stream_event`: locate the session's state by sessionId and
   * reduce its AgentEvent. Late events for deleted sessions are dropped
   * without recreating entries.
   */
  private handleStreamEvent(payload: AgentStreamEventPayload): void {
    const { sessionId, event } = payload;
    if (this.deletedSessions.has(sessionId)) {
      return;
    }
    this.reduceEvent(sessionId, event);
  }

  /**
   * Core reducer: mirrors the streaming contract of message.svelte.ts, but
   * keyed by sessionId.
   */
  private reduceEvent(sessionId: string, event: AgentEvent): void {
    const state = this.ensureState(sessionId);

    switch (event.type) {
      case "agent_start":
        // New run: mark running and clear leftover streaming state.
        state.isRunning = true;
        state.streamingText = "";
        state.thinkingText = "";
        state.error = null;
        break;

      case "message_start":
        // A message begins (user / assistant / toolResult): append to the
        // committed sequence.
        this.appendMessage(sessionId, event.message);
        // Clear streaming accumulators so a new assistant message does not
        // blend with the previous one.
        if (event.message.role === "assistant") {
          state.streamingText = "";
          state.thinkingText = "";
        }
        break;

      case "message_update":
        // Streaming deltas update text/thinking only; tool events are handled
        // by the tool_execution_* handlers.
        this.applyAssistantDelta(sessionId, event.assistantMessageEvent);
        break;

      case "message_end":
        // A message ends: overwrite its committed entry with the final payload
        // and clear streaming accumulators.
        this.finalizeMessage(sessionId, event.message);
        state.streamingText = "";
        state.thinkingText = "";
        break;

      case "agent_end":
        // NEVER overwrite the committed sequence with event.messages:
        // hand-agent's AgentEnd.messages carries only this turn's new messages
        // (user + assistant), not the seeded history transcript. The committed
        // sequence is already maintained incrementally by
        // message_start/message_end as [history + this turn]; overwriting here
        // would collapse multi-turn history to just this turn's two messages
        // until reload. So agent_end only clears streaming leftovers and never
        // touches state.messages.
        state.streamingText = "";
        state.thinkingText = "";
        break;

      case "tool_execution_start":
        this.startToolCall(
          sessionId,
          event.toolCallId,
          event.toolName,
          event.args,
        );
        break;

      case "tool_execution_update":
        this.updateToolCall(
          sessionId,
          event.toolCallId,
          event.toolName,
          event.args,
          event.partialResult.content,
        );
        break;

      case "tool_execution_end":
        this.endToolCall(
          sessionId,
          event.toolCallId,
          event.toolName,
          event.result.content,
          event.isError,
        );
        break;

      // turn_start / turn_end are not consumed; cards are driven by message +
      // tool_execution events.
      default:
        break;
    }
  }

  /**
   * `tool_execution_start`: create the entry keyed by `toolCallId` (if it
   * already exists, keep its final status/result and only refresh known
   * fields — defends against duplicate starts). New entries begin as
   * `executing` with the result pending.
   */
  private startToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args,
        status: existing?.status ?? "executing",
        result: existing?.result,
      },
    };
  }

  /**
   * `tool_execution_update`: update the same `toolCallId` entry's partial
   * result in place, staying `executing` (same card, never a new one). If the
   * entry is missing (update raced ahead of start), create it with start
   * semantics.
   */
  private updateToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
    partialResult: ToolResultContent[],
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args: existing?.args ?? args,
        status: "executing",
        result: partialResult,
      },
    };
  }

  /**
   * `tool_execution_end`: flip the same `toolCallId` entry to its final state
   * (`isError` → `error`, else `completed`) and store the final result — the
   * card flips in place from executing (no new card).
   */
  private endToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    result: ToolResultContent[],
    isError: boolean,
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args: existing?.args,
        status: isError ? "error" : "completed",
        result,
      },
    };
  }

  /**
   * Apply an assistant streaming delta. Only text_delta / thinking_delta are
   * handled; other deltas (toolcall_* etc.) are ignored.
   */
  private applyAssistantDelta(
    sessionId: string,
    delta: AssistantMessageEvent,
  ): void {
    const state = this.ensureState(sessionId);
    switch (delta.type) {
      case "text_delta":
        state.streamingText += delta.delta;
        break;
      case "thinking_delta":
        state.thinkingText += delta.delta;
        break;
      default:
        break;
    }
  }

  private appendMessage(sessionId: string, message: AgentMessage): void {
    const state = this.ensureState(sessionId);
    state.messages = [...state.messages, message];
  }

  /**
   * Overwrite the last same-role message in the committed sequence with the
   * final payload.
   *
   * The backend emits message_start -> message_update* -> message_end in
   * order, so the finalized message corresponds to the last entry with the
   * same role; append defensively if none is found.
   */
  private finalizeMessage(sessionId: string, message: AgentMessage): void {
    const state = this.ensureState(sessionId);
    for (let i = state.messages.length - 1; i >= 0; i -= 1) {
      if (state.messages[i].role === message.role) {
        const next = [...state.messages];
        next[i] = message;
        state.messages = next;
        return;
      }
    }
    this.appendMessage(sessionId, message);
  }

  /**
   * Safety net: flip every live tool-call entry still `executing` to `error`,
   * so an "executing" card can never stay stuck on a spinner after the run
   * ends.
   *
   * On the normal path the backend abort emits a `tool_execution_end` for
   * in-flight tools (`ToolResult::error("...aborted by caller")`,
   * is_error=true) and `endToolCall` flips them in place — this fallback does
   * not replace that, it covers gaps: if that end event never arrives (abort
   * timing / stream closed early), the dangling card is settled here. Entries
   * already final (`completed`/`error`) stay untouched. With no non-final
   * entries this is a no-op (no gratuitous reference swap). Called exactly at
   * run termination (`agent_stream_closed` / error path).
   */
  private settleDanglingToolCalls(sessionId: string): void {
    const state = this.ensureState(sessionId);
    let mutated = false;
    const next: Record<string, ToolCallView> = {};
    for (const [id, view] of Object.entries(state.toolCalls)) {
      if (view.status === "executing") {
        next[id] = { ...view, status: "error" };
        mutated = true;
      } else {
        next[id] = view;
      }
    }
    if (mutated) {
      state.toolCalls = next;
    }
  }

  /**
   * Dispatch `agent_stream_error`: set the session's error view-state (do not
   * clear isRunning — the closed event that follows is the terminal signal)
   * and settle any in-flight tool cards, so even the error path never leaves
   * a stuck spinner.
   */
  private handleStreamError(payload: AgentStreamErrorPayload): void {
    if (this.deletedSessions.has(payload.sessionId)) {
      // Late error for a deleted session: do not recreate the entry.
      return;
    }
    const state = this.ensureState(payload.sessionId);
    state.error = payload.error?.message ?? "Agent run error";
    this.settleDanglingToolCalls(payload.sessionId);
  }

  /**
   * Dispatch `agent_stream_closed`: clear isRunning (exactly once per run),
   * settle any still-executing live tool-call cards (abort-mid-tool fallback),
   * and notify the registered run-closed callback to refresh sidebar metadata.
   * A throwing callback does not affect this store's termination cleanup.
   */
  private handleStreamClosed(payload: AgentStreamClosedPayload): void {
    if (this.deletedSessions.has(payload.sessionId)) {
      // Run termination for a deleted session: reclaim the tombstone (no more
      // events after closed) without recreating state or triggering the
      // sidebar refresh callback.
      this.deletedSessions.delete(payload.sessionId);
      return;
    }
    const state = this.ensureState(payload.sessionId);
    state.isRunning = false;
    // Settle in-flight tool cards on run end (whatever the cause).
    this.settleDanglingToolCalls(payload.sessionId);
    if (this.onRunClosed) {
      try {
        this.onRunClosed(payload.sessionId);
      } catch (error) {
        console.error("Agent run-closed callback failed:", error);
      }
    }
  }

  /**
   * Dispatch `agent_session_lifecycle` (compaction / session-info signals).
   *
   *  - `compaction_start` / `compaction_end`: toggle the session's
   *    `isCompacting` flag; the timeline shows/hides the "compacting"
   *    indicator. Compaction happens within a turn and emits no extra closed
   *    event, so neither `isRunning` nor the closed-once contract is touched
   *    here. `summary` is intentionally not consumed: never rendered into the
   *    timeline, never added to the transcript.
   *  - `session_info_changed`: notify the registered session-name callback so
   *    the sidebar title updates immediately, without this store depending on
   *    session state (one-way wiring).
   *
   * Late signals for deleted sessions are dropped without recreating entries
   * (same tombstone guard as run events). Compaction goes through
   * `ensureState`; session-info creates no run state (it only drives the
   * sidebar).
   */
  private handleLifecycle(payload: AgentSessionLifecyclePayload): void {
    if (this.deletedSessions.has(payload.sessionId)) {
      return;
    }
    switch (payload.kind) {
      case "compaction_start": {
        const state = this.ensureState(payload.sessionId);
        state.isCompacting = true;
        break;
      }
      case "compaction_end": {
        const state = this.ensureState(payload.sessionId);
        state.isCompacting = false;
        // summary intentionally not consumed — just clear the indicator; the
        // turn resumes.
        break;
      }
      case "session_info_changed": {
        if (this.onSessionInfoChanged) {
          try {
            this.onSessionInfoChanged(payload.sessionId, payload.name);
          } catch (error) {
            console.error("Agent session-info callback failed:", error);
          }
        }
        break;
      }
      default:
        break;
    }
  }

  /**
   * Reactive getter: the session's run view-model (the shared read-only empty
   * state when absent). READ-ONLY: never create state here — it is consumed by
   * `$derived` / template expressions, and writing `$state` there throws
   * Svelte's state_unsafe_mutation and crashes the whole render. Real
   * per-session state is lazily created by write paths (`loadTranscript` /
   * event reduce) in non-reactive contexts; reading a missing key still
   * registers the dependency, so this getter recomputes once the state exists.
   */
  runStateFor(sessionId: string): AgentRunState {
    return this.states[sessionId] ?? EMPTY_RUN_STATE;
  }

  /**
   * Normalize an assistant `toolcall` content block into the `ToolCallView`
   * cards consume, reconciling the live and restored sources:
   *  - during a run: the live `state.toolCalls[id]` wins (carries the
   *    real-time executing → final status);
   *  - after reload: live is absent, normalize from the paired committed
   *    `toolResult` content (restored path);
   *  - both absent: only the `toolcall` block exists (result not yet arrived /
   *    persisted) — present as executing.
   *
   * One `toolCallId` maps to one card, live or restored. `committedResult` is
   * paired by toolCallId from the committed transcript by the timeline.
   */
  toolCallViewFor(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
    committedResult?: { content: ToolResultContent[]; isError: boolean },
  ): ToolCallView {
    const live = this.states[sessionId]?.toolCalls[toolCallId];
    if (live) {
      // Live entry wins for status/result, but backfill name/args (a live end
      // event may lack args; the toolcall block always has them).
      return {
        toolCallId,
        toolName: live.toolName || toolName,
        args: live.args ?? args,
        status: live.status,
        result: live.result,
      };
    }
    if (committedResult) {
      // Restored: normalize the paired toolResult into a final state.
      return {
        toolCallId,
        toolName,
        args,
        status: committedResult.isError ? "error" : "completed",
        result: committedResult.content,
      };
    }
    // Only the toolcall block, no result: still executing (not ended, or the
    // result was not persisted).
    return {
      toolCallId,
      toolName,
      args,
      status: "executing",
      result: undefined,
    };
  }

  isRunning(sessionId: string): boolean {
    return this.states[sessionId]?.isRunning ?? false;
  }

  /**
   * Register the run-closed callback, invoked with the session id each time
   * `agent_stream_closed` arrives so the sidebar layer can refresh
   * messageCount / lastMessageAt / ordering. Singleton semantics: only the
   * last registration is kept.
   */
  setOnRunClosed(callback: (sessionId: string) => void): void {
    this.onRunClosed = callback;
  }

  /**
   * Register the session-name-change callback, invoked with the session id
   * and new name (possibly null) each time a `session_info_changed` lifecycle
   * signal arrives, so the sidebar can update the title immediately.
   * Singleton semantics: only the last registration is kept.
   */
  setOnSessionInfoChanged(
    callback: (sessionId: string, name: string | null) => void,
  ): void {
    this.onSessionInfoChanged = callback;
  }

  /**
   * Load and seed a session's committed transcript (called when opening a
   * session), in ascending seq order. The backend `list_messages` returns the
   * full transcript (`ORDER BY seq ASC`, no LIMIT / paging), so long
   * transcripts restore completely without silent truncation.
   *
   * Never overwrites in-flight streaming accumulation; only writes the
   * committed message sequence.
   *
   * Restore means "rebuild from storage": besides writing the committed
   * sequence, stale live `toolCalls` are dropped when there is no active run.
   * The store singleton is navigation-resilient and survives opening/closing
   * sessions, so live tool-call entries from a previous run persist; if kept,
   * the live branch of `toolCallViewFor` would take precedence over the
   * paired committed `toolResult`, making cards read stale live results
   * instead of persisted ones. So with no active run, `toolCalls` is cleared
   * and the restored path rebuilds final-state cards from paired
   * `toolResult`s. With a run in progress (reload landing on an active
   * session), live entries are kept so real-time cards are not clobbered.
   *
   * Robustness: payload parsing is isolated per row — a payload with an
   * unrecognizable shape (not user / assistant / toolResult, or missing the
   * discriminator) is logged and skipped, so one bad row never blanks the
   * whole timeline; the rest render normally.
   */
  async loadTranscript(sessionId: UUID): Promise<void> {
    // Already restored: the store keeps messages per sessionId and run events
    // maintain them incrementally, so revisits need no IPC refetch. Refetching
    // and wholesale-replacing `messages` (new array + new objects) would force
    // AgentTimeline to fully re-render every message (markdown/highlight/
    // katex), making session switches visibly janky. Cold start (not yet
    // hydrated) still restores normally.
    if (this.states[sessionId]?.hydrated) {
      return;
    }
    try {
      const rows: AgentSessionMessage[] =
        await getAgentSessionMessages(sessionId);
      const messages: AgentMessage[] = [];
      for (const row of rows) {
        const parsed = this.parseTranscriptRow(row);
        if (parsed) {
          messages.push(parsed);
        }
      }
      const state = this.ensureState(sessionId);
      state.messages = messages;
      // Notices anchor to live message indices; a wholesale restore renumbers
      // them, so stale entries are dropped rather than mis-anchored.
      state.hookNotices = [];
      // First restore complete: the session page switches from the spinner to
      // real content (or the empty state for an empty session).
      state.hydrated = true;
      // With no active run, drop stale live tool-calls so cards rebuild purely
      // from paired committed toolResults.
      if (!state.isRunning) {
        state.toolCalls = {};
      }
    } catch (error) {
      console.error("Failed to load agent transcript:", error);
      const state = this.ensureState(sessionId);
      state.error = error instanceof Error ? error.message : "加载会话记录失败";
    }
  }

  /**
   * Validate and normalize one transcript row's payload. Returns a valid
   * `AgentMessage`, or `null` (caller skips the row) when the payload is
   * missing / unrecognizable / throws during parsing.
   */
  private parseTranscriptRow(row: AgentSessionMessage): AgentMessage | null {
    try {
      const payload = row?.payload as unknown;
      if (!payload || typeof payload !== "object") {
        console.warn(
          `Skipping corrupt agent transcript row (non-object payload, seq=${row?.seq}).`,
        );
        return null;
      }
      const role = (payload as { role?: unknown }).role;
      if (role !== "user" && role !== "assistant" && role !== "toolResult") {
        console.warn(
          `Skipping corrupt agent transcript row (unknown role=${String(role)}, seq=${row?.seq}).`,
        );
        return null;
      }
      return payload as AgentMessage;
    } catch (error) {
      console.warn(
        `Skipping corrupt agent transcript row (seq=${row?.seq}):`,
        error,
      );
      return null;
    }
  }

  /**
   * Abort the session's active run (passes through to the backend; a clean
   * no-op when there is no active run).
   */
  async abort(sessionId: UUID): Promise<void> {
    try {
      await abortAgentRun(sessionId);
    } catch (error) {
      console.error("Failed to abort agent run:", error);
      throw error;
    }
  }

  /**
   * Cleanup after a session is deleted: drop its run state and add a
   * tombstone to intercept late stream events from an in-flight run.
   *
   * The backend `agent_session_delete` aborts before deleting; the abort's
   * trailing events (tool_execution_end / agent_stream_closed, …) may reach
   * the frontend after the delete completes. Unintercepted, they would
   * recreate the deleted entry via `ensureState` and the run-closed callback
   * would refetch a deleted session (NOT_FOUND console noise). Callers invoke
   * this after a successful delete. The tombstone is reclaimed naturally by
   * the session's `agent_stream_closed`.
   */
  removeSession(sessionId: string): void {
    delete this.states[sessionId];
    this.deletedSessions.add(sessionId);
  }
}

// Singleton; construction sets up the navigation-resilient listener.
export const agentRunStore = new AgentRunStore();

// One-time app-level wiring (navigation-resilient, like the listener): each
// run termination (`agent_stream_closed`) refreshes that session's sidebar
// metadata / ordering. Dependency is one-way (agentRun -> agentSession);
// agentSession never imports this module.
agentRunStore.setOnRunClosed((sessionId) => {
  void agentSessionActions.refreshAfterRun(sessionId);
});

// One-time wiring: when the session name changes via the `session_info_changed`
// lifecycle signal, update the sidebar title immediately. Local state only, no
// network request; same one-way wiring.
agentRunStore.setOnSessionInfoChanged((sessionId, name) => {
  agentSessionActions.applySessionName(sessionId, name);
});
