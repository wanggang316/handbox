/**
 * Default-model resolution shared by every "a persisted model pointer decides
 * what a fresh session runs on" feature (agent sessions, quick action).
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
import type { AgentSettings } from "../types/settings";

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
 * Resolve the app-wide default model (settings > Agent) against the catalog.
 *
 * The single default every session-creating surface reads: the agent session
 * list, the quick-action overlay and the selection window all start on it.
 *
 * @param agentSettings `settingsState.settings?.agent`, or `undefined`/`null`
 *   while settings are still loading.
 * @param allModels the provider+model catalog (`getAllModels()`).
 */
export function resolveAgentDefaultModel(
  agentSettings: AgentSettings | undefined | null,
  allModels: ModelWithProvider[],
): DefaultModelResolution {
  return resolveDefaultModel(
    agentSettings
      ? {
          modelId: agentSettings.defaultModelId,
          providerId: agentSettings.defaultProviderId,
        }
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
