/**
 * Default-model resolution shared by every "a persisted model pointer decides
 * what a fresh session runs on" feature (agent sessions, quick action).
 *
 * The pointer lives on the AGENT DEFINITION every session is instantiated from;
 * it used to be a single app-wide setting.
 *
 * A stored default is a `(modelId, providerId)` PAIR: the same model id can
 * exist under several providers, so neither half is meaningful alone. The pair
 * is resolved against the live catalog because a provider can be disabled or a
 * model removed after the default was picked — a dangling pointer must degrade
 * to "pick a model", never to a session that cannot run.
 *
 * Kept PURE (catalog + settings passed in) so it is unit-testable without
 * mounting the provider/settings stores.
 */

import type { ModelWithProvider } from "../types/provider";
import type { InstantiateAgentSessionRequest } from "../types/agentSession";
import type { Agent } from "../types/agent";

/** Why a stored default cannot produce a runnable model. */
export type DefaultModelEmptyReason =
  | "empty-catalog" // no enabled provider+model exists at all
  | "no-default" // the user has not picked a default model yet
  | "dangling-default"; // a default was set but is no longer in the catalog

/** A resolved, runnable default model. */
export interface DefaultModelResolved {
  available: true;
  modelId: string;
  providerId: string;
  model: ModelWithProvider;
}

/** No runnable default; callers show a "configure a model" prompt instead. */
export interface DefaultModelEmpty {
  available: false;
  reason: DefaultModelEmptyReason;
}

export type DefaultModelResolution = DefaultModelResolved | DefaultModelEmpty;

/** The persisted pointer half of any default-model setting. */
export interface DefaultModelPreference {
  modelId?: string | null;
  providerId?: string | null;
}

/**
 * Resolve a stored default-model pointer against the enabled catalog.
 *
 * @param preference the persisted `(modelId, providerId)` pair, or
 *   `undefined`/`null` when the settings slice is unset or still loading.
 * @param allModels the provider+model catalog (`getAllModels()`).
 */
export function resolveDefaultModel(
  preference: DefaultModelPreference | undefined | null,
  allModels: ModelWithProvider[],
): DefaultModelResolution {
  if (allModels.length === 0) {
    return { available: false, reason: "empty-catalog" };
  }

  const modelId = preference?.modelId;
  const providerId = preference?.providerId;
  if (!modelId || !providerId) {
    return { available: false, reason: "no-default" };
  }

  // The Model type uses snake_case `provider_id` while settings store camelCase
  // `providerId`; matching on both halves mirrors AgentInput's lookup.
  const model = allModels.find(
    (m) => m.id === modelId && m.provider_id === providerId,
  );
  if (!model) {
    return { available: false, reason: "dangling-default" };
  }

  return { available: true, modelId, providerId, model };
}

/**
 * Resolve an agent definition's default model against the catalog.
 *
 * The default every session-creating surface reads: the session list, the
 * quick-action overlay and the selection window all instantiate from a
 * definition, so the definition is what says which model they start on. It
 * replaced an app-wide setting, which could not say "the coding agent runs on a
 * big model, the quick translator on a cheap one".
 *
 * @param agent the source definition, or `undefined`/`null` while it loads.
 * @param allModels the provider+model catalog (`getAllModels()`).
 */
export function resolveAgentDefaultModel(
  agent: Agent | undefined | null,
  allModels: ModelWithProvider[],
): DefaultModelResolution {
  return resolveDefaultModel(
    agent
      ? { modelId: agent.defaultModelId, providerId: agent.defaultProviderId }
      : null,
    allModels,
  );
}

/**
 * Fill a session-instantiation request's model with the resolved default.
 *
 * An explicit pair in `overrides` always wins (the quick-action overlay and the
 * selection window resolve their own model). A half-set pair is treated as
 * unset, since the backend needs both to run. An unresolvable default is left
 * alone: the session is created model-less and the composer prompts for one,
 * which beats writing a model id that no longer exists.
 */
export function applyDefaultModel(
  overrides: InstantiateAgentSessionRequest | undefined,
  resolution: DefaultModelResolution,
): InstantiateAgentSessionRequest | undefined {
  if (overrides?.modelId && overrides.providerId) return overrides;
  if (!resolution.available) return overrides;
  return {
    ...overrides,
    modelId: resolution.modelId,
    providerId: resolution.providerId,
  };
}
