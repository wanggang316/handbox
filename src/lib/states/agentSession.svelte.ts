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
import type { AgentSessionField, TitleScope } from "../api/agentSession";
import type { TitleGenerationRule } from "../types/settings";
import * as agentSessionApi from "../api/agentSession";
import { normalizeError } from "../utils/error";
import {
  applyDefaultModel,
  resolveAgentDefaultModel,
} from "../utils/defaultModel";
import { agentState } from "./agent.svelte";
import { settingsState } from "./settings.svelte";
import { getAllModels, providerActions } from "./provider.svelte";

let sessions = $state<AgentSession[]>([]);
let currentSession = $state<AgentSession | null>(null);
let isLoading = $state(false);

// Session ids whose title was auto-generated (once per session; removed on
// failure to allow a retry).
const autoTitledSessions = new Set<string>();

// Session ids the user renamed by hand. Under the "every message" rule their
// title is left alone. In-memory only: after a restart such a session resumes
// auto-titling, which is what that rule promises.
const manuallyRenamedSessions = new Set<string>();

/** Falls back to the historical behaviour while settings are still loading. */
function titleGenerationRule(): TitleGenerationRule {
  return settingsState.settings?.session?.titleGeneration ?? "firstMessage";
}

/**
 * Stamp the configured default model (settings > Agent) onto instantiation
 * overrides that do not pin one, so a freshly created session is runnable
 * without opening the model picker.
 *
 * Definitions carry no model, so without this every new session starts blank.
 * Callers that resolved their own model (quick action, selection) pass the pair
 * explicitly and are left untouched. The catalog is loaded on demand because
 * helper windows skip the main window's preload; a dangling or unset default
 * simply leaves the session model-less.
 */
async function withDefaultModel(
  overrides?: InstantiateAgentSessionRequest,
): Promise<InstantiateAgentSessionRequest | undefined> {
  if (overrides?.modelId && overrides.providerId) return overrides;

  const preference = settingsState.settings?.agent;
  if (!preference?.defaultModelId || !preference.defaultProviderId) {
    return overrides;
  }

  if (getAllModels().length === 0) {
    try {
      await providerActions.loadProvidersWithModels();
    } catch (error) {
      // Catalog unavailable: fall through and create the session model-less.
      console.error("Failed to load model catalog for default model:", error);
    }
  }

  return applyDefaultModel(
    overrides,
    resolveAgentDefaultModel(preference, getAllModels()),
  );
}

/**
 * Auto-generate a title after a run, per the `session.titleGeneration` rule:
 *
 * - `off`: never.
 * - `firstMessage`: once, after the first run, and only while the name still
 *   equals the source agent's default name (i.e. the user has not renamed it).
 * - `everyMessage`: after every run, re-titled from the conversation so far,
 *   unless the user renamed the session by hand.
 *
 * Runs in the background and fails silently — the user can always generate
 * manually via the context menu.
 */
async function maybeAutoGenerateTitle(
  id: string,
  wasFirstRun: boolean,
  currentName: string,
  agentDefinitionId?: string,
): Promise<void> {
  const rule = titleGenerationRule();
  if (rule === "off") return;

  let scope: TitleScope = "firstMessage";
  if (rule === "firstMessage") {
    if (!wasFirstRun || autoTitledSessions.has(id)) return;
    const agent = agentDefinitionId
      ? agentState.agents.find((a) => a.id === agentDefinitionId)
      : undefined;
    // Source agent unresolvable (missing / dangling agentDefinitionId), or the
    // name is no longer the default: do not auto-rename.
    if (!agent || agent.name !== currentName) return;
  } else {
    if (manuallyRenamedSessions.has(id)) return;
    // A single message can only be read as the whole conversation, so the
    // first run stays on the cheaper first-message prompt.
    scope = wasFirstRun ? "firstMessage" : "conversation";
  }

  autoTitledSessions.add(id);
  try {
    await agentSessionActions.generateTitle(id, scope);
  } catch (error) {
    autoTitledSessions.delete(id);
    console.warn("Auto title generation failed:", error);
  }
}

/** Writes a session object into the list and, when it is current, there too. */
function applySession(id: UUID, session: AgentSession): void {
  const index = sessions.findIndex((item) => item.id === id);
  if (index !== -1) {
    sessions[index] = session;
  }
  if (currentSession?.id === id) {
    currentSession = session;
  }
}

/**
 * Optimistically merges sidebar flags into the local session and returns the
 * undo. A session that is not (or no longer) listed yields a no-op undo, so a
 * failed toggle on a deleted session cannot resurrect it.
 */
function applySessionFlags(
  id: UUID,
  patch: { pinned?: boolean; archived?: boolean },
): () => void {
  const previous = sessions.find((session) => session.id === id);
  if (!previous) return () => {};
  applySession(id, { ...previous, ...patch });
  return () => {
    if (sessions.some((session) => session.id === id)) {
      applySession(id, previous);
    }
  };
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
   * `overrides` only covers name/project/workingDir/model/provider. An
   * override without a model pair inherits the configured default model.
   */
  async createSessionFromDefinition(
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    try {
      isLoading = true;
      const session = await agentSessionApi.createSessionFromDefinition(
        definitionId,
        await withDefaultModel(overrides),
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
   *
   * The re-snapshot drops the model along with the rest of the old parameters,
   * so the session's current model is carried over when the caller pins none —
   * switching agents must not silently blank the composer's model.
   */
  async reinstantiateFromDefinition(
    sessionId: UUID,
    definitionId: UUID,
    overrides?: InstantiateAgentSessionRequest,
  ): Promise<AgentSession> {
    const previous =
      sessions.find((session) => session.id === sessionId) ??
      (currentSession?.id === sessionId ? currentSession : undefined);
    const carried: InstantiateAgentSessionRequest = {
      modelId: previous?.modelId,
      providerId: previous?.providerId,
      ...overrides,
    };
    const updated = await agentSessionApi.reinstantiateSessionFromDefinition(
      sessionId,
      definitionId,
      await withDefaultModel(carried),
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

  /** Manual rename: also opts the session out of "every message" auto-titling. */
  async renameSession(id: UUID, name: string): Promise<void> {
    const updated = await agentSessionApi.renameAgentSession(id, name);
    manuallyRenamedSessions.add(id);
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
  async generateTitle(id: UUID, scope?: TitleScope): Promise<AgentSession> {
    const updated = await agentSessionApi.generateAgentSessionTitle(id, scope);
    const index = sessions.findIndex((session) => session.id === id);
    if (index !== -1) {
      sessions[index] = updated;
    }
    if (currentSession?.id === id) {
      currentSession = updated;
    }
    return updated;
  },

  /**
   * Pin / unpin a session. Optimistic: the flag flips locally first so the row
   * reorders under the cursor without a round-trip, then the backend's returned
   * session replaces the entry. A failure rolls the flag back and rethrows so
   * the caller can surface it.
   */
  async setPinned(id: UUID, pinned: boolean): Promise<void> {
    const rollback = applySessionFlags(id, { pinned });
    try {
      applySession(id, await agentSessionApi.setAgentSessionPinned(id, pinned));
    } catch (error) {
      rollback();
      console.error("Failed to pin agent session:", error);
      throw error;
    }
  },

  /** Archive / unarchive a session; same optimistic contract as `setPinned`. */
  async setArchived(id: UUID, archived: boolean): Promise<void> {
    const rollback = applySessionFlags(id, { archived });
    try {
      applySession(
        id,
        await agentSessionApi.setAgentSessionArchived(id, archived),
      );
    } catch (error) {
      rollback();
      console.error("Failed to archive agent session:", error);
      throw error;
    }
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
      void maybeAutoGenerateTitle(
        id,
        wasFirstRun,
        updated.name,
        agentDefinitionId,
      );
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
