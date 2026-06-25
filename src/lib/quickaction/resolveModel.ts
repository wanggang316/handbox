/**
 * Quick-action default-model resolver.
 *
 * Single source of truth for "what model does a fresh quick-action overlay
 * summon use, and is it runnable?". Both the overlay send path and the
 * settings page lean on this.
 *
 * Kept as a PURE function (catalog + settings passed in as arguments) so it
 * is unit-testable without mounting the provider/settings stores. Callers
 * pass `settingsState.settings?.quickAction` and `getAllModels()`.
 */

import type { ModelWithProvider } from "../types/provider";
import type { QuickActionSettings } from "../types/settings";

/**
 * Why a configured quick-action default cannot produce a runnable model.
 * Callers use this to decide which "configure a model" prompt to show.
 */
export type QuickActionEmptyReason =
  | "empty-catalog" // no enabled provider+model exists at all
  | "no-default" // the user has not picked a default model yet
  | "dangling-default"; // a default was set but is no longer in the catalog

/** The overlay has a resolved, runnable default model. */
export interface QuickActionModelResolved {
  available: true;
  modelId: string;
  providerId: string;
  model: ModelWithProvider;
}

/** The overlay has no runnable default; show the configure prompt instead. */
export interface QuickActionModelEmpty {
  available: false;
  reason: QuickActionEmptyReason;
}

export type QuickActionModelResolution =
  | QuickActionModelResolved
  | QuickActionModelEmpty;

/**
 * Resolve the quick-action overlay's effective model against the catalog.
 *
 * @param quickActionSettings the persisted `quickAction` settings slice
 *   (`settingsState.settings?.quickAction`), or `undefined` if unset.
 * @param allModels the enabled provider+model catalog (`getAllModels()`).
 * @returns the resolved model, or an empty-state result describing why no
 *   runnable model is available.
 */
export function resolveQuickActionModel(
  quickActionSettings: QuickActionSettings | undefined | null,
  allModels: ModelWithProvider[],
): QuickActionModelResolution {
  if (allModels.length === 0) {
    return { available: false, reason: "empty-catalog" };
  }

  const modelId = quickActionSettings?.modelId;
  const providerId = quickActionSettings?.providerId;
  if (!modelId || !providerId) {
    return { available: false, reason: "no-default" };
  }

  // Catalog item key mirrors AgentInput's lookup: the Model type uses
  // snake_case `provider_id` while settings store camelCase `providerId`.
  const model = allModels.find(
    (m) => m.id === modelId && m.provider_id === providerId,
  );
  if (!model) {
    return { available: false, reason: "dangling-default" };
  }

  return { available: true, modelId, providerId, model };
}
