/**
 * Quick-action default-model resolver.
 *
 * Single source of truth for "what model does a fresh quick-action overlay
 * summon use, and is it runnable?". Both the overlay send path and the
 * settings page lean on this.
 *
 * The resolution itself is the shared `resolveDefaultModel` helper (the agent
 * settings' default model resolves the same way); this module only binds it to
 * the `quickAction` settings slice.
 *
 * Kept as a PURE function (catalog + settings passed in as arguments) so it
 * is unit-testable without mounting the provider/settings stores. Callers
 * pass `settingsState.settings?.quickAction` and `getAllModels()`.
 */

import type { ModelWithProvider } from "../types/provider";
import type { QuickActionSettings } from "../types/settings";
import {
  resolveDefaultModel,
  type DefaultModelEmptyReason,
  type DefaultModelResolution,
  type DefaultModelResolved,
  type DefaultModelEmpty,
} from "../utils/defaultModel";

/**
 * Why a configured quick-action default cannot produce a runnable model.
 * Callers use this to decide which "configure a model" prompt to show.
 */
export type QuickActionEmptyReason = DefaultModelEmptyReason;

/** The overlay has a resolved, runnable default model. */
export type QuickActionModelResolved = DefaultModelResolved;

/** The overlay has no runnable default; show the configure prompt instead. */
export type QuickActionModelEmpty = DefaultModelEmpty;

export type QuickActionModelResolution = DefaultModelResolution;

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
  return resolveDefaultModel(quickActionSettings, allModels);
}
