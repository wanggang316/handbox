/**
 * Lightweight in-house i18n runtime.
 *
 * - Single source of truth: reads `uiState.language` (Svelte 5 `$state`).
 *   Calling `t(...)` in a template creates a reactive dependency on the
 *   language, so switching locales re-renders in place — no page reload.
 * - Zero deps, type-safe: keys derive from the canonical `zh-CN` dictionary,
 *   so missing translations surface at compile time.
 * - Simple interpolation: `{name}` placeholders are filled from `params`.
 */
import { uiState } from "$lib/states/ui.svelte";
import { dictionaries, type Locale, type MessageKey } from "./locales";

export type { Locale, MessageKey } from "./locales";

export type TranslationParams = Record<string, string | number>;

const FALLBACK_LOCALE: Locale = "zh-CN";

/** Translate a key for the current UI language; falls back to the canonical dictionary, then the key itself. */
export function t(key: MessageKey, params?: TranslationParams): string {
  const locale = uiState.language as Locale;
  const dict = dictionaries[locale] ?? dictionaries[FALLBACK_LOCALE];
  const template = dict[key] ?? dictionaries[FALLBACK_LOCALE][key] ?? key;
  return params ? interpolate(template, params) : template;
}

function interpolate(template: string, params: TranslationParams): string {
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
}
