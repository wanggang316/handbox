/**
 * Pure builder for the quick-action overlay's throwaway agent session.
 *
 * Given the resolved quick-action model (from {@link resolveQuickActionModel})
 * and the global agent default-tools setting, it produces either a ready-to-send
 * `CreateAgentSessionRequest` or an empty-state decision telling the overlay to
 * show the "configure a model" prompt instead of creating an unrunnable session.
 *
 * Kept PURE (no store access, no `.svelte` imports) so the model/default/empty
 * decisions are unit-testable without mounting the provider/settings stores.
 * The overlay passes `resolveQuickActionModel(...)`, `settings.agent.defaultEnabledTools`
 * and a localized session name in; this function does no I/O.
 */

import type { CreateAgentSessionRequest } from "../types/agentSession";
import { BUILTIN_TOOL_IDS } from "../constants/builtinToolIds";
import type {
  QuickActionModelResolution,
  QuickActionEmptyReason,
} from "./resolveModel";

/** The overlay should create a session with this request. */
export interface QuickSessionRequestReady {
  status: "ready";
  request: CreateAgentSessionRequest;
}

/** No runnable model — show the configure prompt; do NOT create a session. */
export interface QuickSessionRequestEmpty {
  status: "empty";
  reason: QuickActionEmptyReason;
}

export type QuickSessionRequestDecision =
  | QuickSessionRequestReady
  | QuickSessionRequestEmpty;

/** Inputs the overlay supplies to the builder. */
export interface BuildQuickSessionInput {
  /** Result of {@link resolveQuickActionModel} (in-panel pick or quick-action default). */
  resolution: QuickActionModelResolution;
  /** `settings.agent.defaultEnabledTools`, or `undefined`/`null` when settings unloaded. */
  defaultEnabledTools: string[] | undefined | null;
  /** Localized session name (e.g. `t("quickaction.sessionName")`). */
  name: string;
}

/**
 * Build the create-session request for a fresh quick-action summon.
 *
 * @returns `{status:"ready", request}` when a runnable model resolved, or
 *   `{status:"empty", reason}` (mirrors the resolver's empty reason) when no
 *   runnable model is available — in which case the overlay must NOT create a
 *   session and instead show the configure prompt.
 *
 * The request mirrors `AgentProjectList.handleCreateSessionInProject`:
 *  - `modelId` / `providerId` come from the resolution (in-panel pick or default);
 *  - `enabledTools` = `defaultEnabledTools`, falling back to all 7 built-ins when
 *    settings have not loaded;
 *  - NO `projectId` → the backend gives the session a sandbox working dir.
 */
export function buildQuickSessionRequest(
  input: BuildQuickSessionInput,
): QuickSessionRequestDecision {
  const { resolution, defaultEnabledTools, name } = input;

  if (!resolution.available) {
    return { status: "empty", reason: resolution.reason };
  }

  const enabledTools =
    defaultEnabledTools && defaultEnabledTools.length > 0
      ? [...defaultEnabledTools]
      : [...BUILTIN_TOOL_IDS];

  return {
    status: "ready",
    request: {
      name,
      modelId: resolution.modelId,
      providerId: resolution.providerId,
      enabledTools,
      // No projectId — backend assigns a sandbox cwd for a throwaway session.
    },
  };
}
