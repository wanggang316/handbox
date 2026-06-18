/**
 * 轻量自研 i18n 运行时。
 *
 * 设计要点：
 * - 单一事实来源——直接读取 `uiState.language`（Svelte 5 `$state`）。
 *   在组件模板里调用 `t(...)` 会建立对 language 的响应式依赖，
 *   切换语言即原地重渲染，无需整页刷新。
 * - 零依赖、类型安全：key 由权威词典 `zh-CN` 推导，漏译在编译期暴露。
 * - 简单插值：模板中的 `{name}` 占位由 `params` 替换。
 */
import { uiState } from "$lib/states/ui.svelte";
import { dictionaries, type Locale, type MessageKey } from "./locales";

export type { Locale, MessageKey } from "./locales";

export type TranslationParams = Record<string, string | number>;

const FALLBACK_LOCALE: Locale = "zh-CN";

/**
 * 按当前界面语言翻译一个 key。缺失时回退到权威词典，再回退到 key 本身。
 */
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
