/**
 * Draft agent sessions: what "New chat" produces before anything is persisted.
 *
 * A draft is a full `AgentSession` shape carrying a local id, so every
 * session-driven surface (composer, header) renders it without knowing it is a
 * draft; only the id distinguishes the two, and the first send trades the draft
 * for a real row. Keeping the shape identical is what makes deferred creation a
 * routing decision rather than a second code path through the UI.
 */

import type { Agent } from "../types/agent";
import type { AgentSession } from "../types/agentSession";

/**
 * Marks an id as local-only. Persisted sessions are UUIDs, so no backend id can
 * collide with the prefixed form — an id that reaches the IPC layer by mistake
 * fails loudly as NOT_FOUND rather than mutating some other session.
 */
const DRAFT_ID_PREFIX = "draft:";

/** A fresh local id. Distinct per draft, so the composer remounts between them. */
export function newDraftSessionId(): string {
  return `${DRAFT_ID_PREFIX}${crypto.randomUUID()}`;
}

/** True for a session that exists only in memory: no row, no runnable id. */
export function isDraftSessionId(id: string | null | undefined): boolean {
  return typeof id === "string" && id.startsWith(DRAFT_ID_PREFIX);
}

export interface DraftSessionSeed {
  /** Reused across re-seeds (agent switch) so the composer keeps its state. */
  id: string;
  definitionId: string;
  /** `null` while the agent list is still loading: the draft keeps the id and
   *  the model pair, and the capability fields stay empty until it resolves. */
  definition: Agent | null;
  modelId?: string;
  providerId?: string;
  workingDir?: string;
  /** Project the working dir resolved to; dropped alongside it for `"none"`. */
  projectId?: string;
  now: number;
}

/**
 * Seed a draft from its source definition, mirroring what the backend's
 * instantiate would write: the definition decides the capability snapshot and
 * the parameter defaults, while the model pair comes from the caller because
 * definitions carry none.
 *
 * The working dir follows the same arbitration as create — a `"none"`
 * definition never carries one — so a draft cannot promote into a session the
 * backend would have rejected.
 */
export function buildDraftSession(seed: DraftSessionSeed): AgentSession {
  const definition = seed.definition;
  const takesDirectory = definition?.workingDirMode !== "none";
  const workingDir = takesDirectory ? seed.workingDir : undefined;

  return {
    id: seed.id,
    projectId: takesDirectory ? seed.projectId : undefined,
    agentDefinitionId: seed.definitionId,
    name: definition?.name ?? "",
    modelId: seed.modelId,
    providerId: seed.providerId ?? definition?.providerId ?? undefined,
    systemPrompt: definition?.systemPrompt,
    thinkingLevel: definition?.thinkingLevel ?? undefined,
    temperature: definition?.temperature,
    maxTokens: definition?.maxTokens,
    workingDir,
    enabledTools: definition?.builtinTools ? [...definition.builtinTools] : [],
    mcpServers: definition?.mcpServers ? [...definition.mcpServers] : [],
    toolExecutionMode: definition?.toolExecutionMode ?? undefined,
    messageCount: 0,
    pinned: false,
    archived: false,
    createdAt: seed.now,
    updatedAt: seed.now,
  };
}
