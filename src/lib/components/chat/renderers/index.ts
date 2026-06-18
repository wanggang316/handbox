/**
 * Runtime entry for the dynamic renderer pipeline.
 *
 * This is the only module in the directory that imports a `.svelte` component,
 * so it deliberately lives outside the unit-test import graph (the tests target
 * the pure-TS modules — `registry.ts`, `translation.ts` — under a Node
 * environment that cannot compile Svelte).
 *
 * It binds the built-in renderers (currently just `translation`, backed by
 * {@link TranslationCard}) onto the shared {@link rendererRegistry} and re-exports
 * it for downstream consumers (e.g. the message-rendering feature).
 */

import type { Component } from "svelte";
import { rendererRegistry } from "./registry";
import { validateTranslation } from "./translation";
import type { Renderer, TranslationData } from "./types";
import TranslationCard from "./TranslationCard.svelte";

/** The `translation` renderer: validated by {@link validateTranslation}, drawn by {@link TranslationCard}. */
export const translationRenderer: Renderer<TranslationData, Component<TranslationData>> = {
  type: "translation",
  validate: validateTranslation,
  component: TranslationCard,
};

rendererRegistry.register(translationRenderer);

export { rendererRegistry };
