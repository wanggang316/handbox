/**
 * Agent session state - Svelte 5 runes.
 *
 * Follows the project's store conventions: module-level `$state` variables +
 * a getter/setter state object + one actions object. Handles only session
 * CRUD and list interactions; run / timeline state lives in agentRun.
 */

import type {
  UUID,
  AgentSession,
  CreateAgentSessionRequest,
  InstantiateAgentSessionRequest,
} from "../types";
import type { McpServerConfig } from "../types/llm";
import type { AgentSessionField } from "../api/agentSession";
import * as agentSessionApi from "../api/agentSession";
import { normalizeError } from "../utils/error";
import { agentState } from "./agent.svelte";

let sessions = $state<AgentSession[]>([]);
let currentSession = $state<AgentSession | null>(null);
let isLoading = $state(false);

// Session ids whose title was auto-generated (once per session; removed on
// failure to allow a retry).
const autoTitledSessions = new Set<string>();

/**
 * Auto-generate a title after a session's first run, only when: it is the
 * first run, no auto-generation happened yet, and the current name still
 * equals the source agent's default name (i.e. the user has not renamed it).
 * Runs in the background and fails silently — the user can always generate
 * manually via the context menu. A manually set title is never overwritten.
 */
async function maybeAutoGenerateTitle(
  id: string,
  wasFirstRun: boolean,
  currentName: string,
  agentDefinitionId?: string,
): Promise<void> {
  if (!wasFirstRun || autoTitledSessions.has(id)) return;
  const agent = agentDefinitionId
    ? agentState.agents.find((a) => a.id === agentDefinitionId)
    : undefined;
  // Source agent unresolvable (missing / dangling agentDefinitionId), or the
  // name is no longer the default: do not auto-rename.
  if (!agent || agent.name !== currentName) return;

  autoTitledSessions.add(id);
  try {
    await agentSessionActions.generateTitle(id);
  } catch (error) {
    autoTitledSessions.delete(id);
    console.warn("Auto title generation failed:", error);
  }
}

export const agentSessionState = {
  get sessions() {
    return sessions;
  },
  set sessions(value) {
    sessions = value;
  },

  get currentSession() {
    return currentSession;
  },
  set currentSession(value) {
    currentSession = value;
  },

  get isLoading() {
    return isLoading;
  },
  set isLoading(value) {
    isLoading = value;
  },
};

export const agentSessionActions = {
  /** Load the session list (backend returns updatedAt DESC; order kept as-is). */
  async loadSessions(): Promise<void> {
    try {
      isLoading = true;
      sessions = await agentSessionApi.getAgentSessions();
    } catch (error) {
      console.error("Failed to load agent sessions:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /** Create a new agent session: insert at the top and set as current. */
  async createSession(
    config: CreateAgentSessionRequest,
  ): Promise<AgentSession> {
    try {
      isLoading = true;
      const session = await agentSessionApi.createAgentSession(config);
      const existing = Array.isArray(sessions) ? sessions : [];
      sessions = [session, ...existing];
      currentSession = session;
      return session;
    } catch (error) {
      console.error("Failed to create agent session:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * Instantiate a session from an AgentDefinition: insert at the top and set
   * as current.
   *
   * The single "use this agent" entry point. The capability snapshot and
   * working-dir policy are decided by the backend from the definition;
   * `overrides` only covers name/project/workingDir/model/provider.
   */
  async createSessionFromDefinition(
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    try {
      isLoading = true;
      const session = await agentSessionApi.createSessionFromDefinition(
        definitionId,
        overrides,
      );
      const existing = Array.isArray(sessions) ? sessions : [];
      sessions = [session, ...existing];
      currentSession = session;
      return session;
    } catch (error) {
      console.error("Failed to create agent session from definition:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * Re-point an existing session at another AgentDefinition in place (no new
   * session).
   *
   * Used to switch agents while the current session has no messages yet:
   * reuses the session id while the backend re-snapshots capabilities and
   * parameters and rewrites provenance. Replaces the list entry in place and
   * syncs the current session (same id, no reorder).
   */
  async reinstantiateFromDefinition(
    sessionId: UUID,
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    const updated = await agentSessionApi.reinstantiateSessionFromDefinition(
      sessionId,
      definitionId,
      overrides,
    );
    const index = sessions.findIndex((session) => session.id === sessionId);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === sessionId) {
      currentSession = updated;
    }
    return updated;
  },

  async renameSession(id: UUID, name: string): Promise<void> {
    const updated = await agentSessionApi.renameAgentSession(id, name);
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
  },

  /**
   * Generate a session title (backend one-shot LLM completion + persist) and
   * write the returned session back to the list and current session. Failures
   * propagate so callers decide whether to notify (auto path stays silent,
   * manual path may toast).
   */
  async generateTitle(id: UUID): Promise<AgentSession> {
    const updated = await agentSessionApi.generateAgentSessionTitle(id);
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
    return updated;
  },

  /** Delete a session: remove from the list; clear current if it was current. */
  async deleteSession(id: UUID): Promise<void> {
    try {
      isLoading = true;
      await agentSessionApi.deleteAgentSession(id);
      sessions = sessions.filter((session) => session.id !== id);
      if (currentSession?.id === id) {
        currentSession = null;
      }
    } catch (error) {
      console.error("Failed to delete agent session:", error);
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /** Update a single session field (syncs the local list and current session). */
  async updateField(
    id: UUID,
    field: AgentSessionField,
    value: string | number | string[] | McpServerConfig[] | null,
  ): Promise<void> {
    const updated = await agentSessionApi.updateAgentSessionField(
      id,
      field,
      value,
    );
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
  },

  /**
   * Apply a session-name change immediately: when the name changes via the
   * backend `SessionInfoChanged{name}` lifecycle signal, `agentRunStore`'s
   * name-change callback invokes this with the session id + new name to update
   * the sidebar list entry and current session title in place, without a
   * reopen / manual refresh.
   *
   * Pure local state update (no network request, no reorder — title only).
   * A session not in the list (e.g. deleted) is a clean no-op. A null `name`
   * (cleared title) falls back to the empty string so the sidebar never
   * renders `undefined` / a broken placeholder.
   */
  applySessionName(id: UUID, name: string | null): void {
    const nextName = name ?? "";
    const index = sessions.findIndex((session) => session.id === id);
    if (index === -1) {
      // Not in the list: clean no-op (no ghost entry).
      return;
    }
    sessions[index] = { ...sessions[index], name: nextName };
    if (currentSession?.id === id) {
      currentSession = { ...currentSession, name: nextName };
    }
  },

  /** Set an already-listed session as current (no network request). */
  setCurrentById(id: UUID): AgentSession | null {
    const session = sessions.find((item) => item.id === id) ?? null;
    currentSession = session;
    return session;
  },

  /**
   * Refresh a session's sidebar metadata after a run ends.
   *
   * During a run the backend appends the transcript on message_end and bumps
   * the session's `messageCount` / `lastMessageAt` / `updatedAt`, but the
   * frontend list holds the pre-run snapshot. When `agent_stream_closed`
   * arrives, this refetches the session, updates the list entry and current
   * session, and moves it to the top per `updatedAt DESC` — so counts /
   * recency / ordering update without a manual refresh.
   *
   * A session not in the list (e.g. already deleted) is a clean no-op; a
   * NOT_FOUND on refetch (abort cleanup racing the delete IPC reply) silently
   * removes the row; other errors are only logged, never thrown, so the rest
   * of run-termination cleanup is unaffected.
   */
  async refreshAfterRun(id: UUID): Promise<void> {
    const prev = sessions.find((session) => session.id === id);
    if (!prev) {
      // Deleted (or never listed) session: silent no-op. Do not refetch —
      // agent_session_get on a deleted id is guaranteed NOT_FOUND and would
      // only leave pointless console.error noise via the catch below.
      return;
    }
    // Capture "was this the first run" before the refetch refreshes
    // messageCount, for the auto-title decision.
    const wasFirstRun = prev.messageCount === 0;
    const agentDefinitionId = prev.agentDefinitionId;
    try {
      const updated = await agentSessionApi.getAgentSession(id);
      const others = sessions.filter((session) => session.id !== id);
      if (others.length === sessions.length) {
        // Deleted during the await: do not insert a ghost entry.
        return;
      }
      // Move to top + refresh metadata (the refetched object carries the
      // backend's latest lastMessageAt; groupSessions sorts it first in its
      // group and floats the group by the activity key).
      sessions = [updated, ...others];
      if (currentSession?.id === id) {
        currentSession = updated;
      }
      // Auto-generate a title after the first run (background, silent on
      // failure, never overwrites a manual title).
      void maybeAutoGenerateTitle(id, wasFirstRun, updated.name, agentDefinitionId);
    } catch (error) {
      if (normalizeError(error).code === "NOT_FOUND") {
        // Session deleted while refreshing (abort-closed raced the delete IPC):
        // drop the stale row silently — no ghost, no console noise.
        sessions = sessions.filter((session) => session.id !== id);
        return;
      }
      console.error("Failed to refresh agent session after run:", error);
    }
  },
};
