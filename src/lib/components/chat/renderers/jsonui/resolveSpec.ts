/**
 * JSON-Render spec resolver.
 *
 * Decides whether an agent message *is* a renderable JSON-Render {@link Spec}
 * and, if so, returns it. Two carriers are accepted (mirroring the envelope
 * parser's "whole message is exactly one JSON payload" discipline):
 *
 *  1. **Bare JSON** — the trimmed message is exactly one JSON object.
 *  2. **A single ```json fenced code block** — the trimmed message is exactly
 *     one fenced block whose language token is `json` (case-insensitive), with
 *     nothing but whitespace around it.
 *
 * Unlike the envelope parser, no `__render` discriminator is required. Instead a
 * candidate qualifies as a spec only when it
 *
 *  (a) structurally looks like a spec — a string `root` plus an object
 *      `elements`;
 *  (b) passes `uiCatalog.validate`, the catalog's structural pass that checks
 *      the spec shape and every element's `type` membership / `children` /
 *      `visible`;
 *  (c) passes per-component prop validation — each element's `props` is parsed
 *      through the catalog's Zod schema for that component type. A missing
 *      required prop, a wrong type, or an illegal enum value fails the spec;
 *      undeclared props are stripped from the resolved data. (The catalog's
 *      multi-component `validate` path itself treats `props` as an opaque record,
 *      so this enforcement is applied explicitly here.)
 *  (d) passes `validateSpec` for reference integrity — the `root` and every
 *      `children` entry must name an existing element.
 *
 * A spec referencing an unregistered component type, carrying invalid props, or
 * with a dangling root/child reference therefore fails and yields `null`.
 *
 * The function never throws: any malformed, non-spec, or non-JSON input — and
 * any internal validation error — yields `null`, signalling the caller to fall
 * back to ordinary markdown rendering.
 */

import type { Spec, UIElement } from "@json-render/core";
import { formatSpecIssues, validateSpec } from "@json-render/core";
import type { ZodError, ZodTypeAny } from "zod";
import { uiCatalog } from "./catalog";

/**
 * The catalog's per-component prop schemas, keyed by component type. The
 * multi-component `uiCatalog.validate` path validates structure only and leaves
 * `props` as an opaque record, so {@link validateElementProps} reaches into this
 * map to enforce (and strip to) each component's declared prop shape.
 */
const componentPropsSchemas: Record<string, ZodTypeAny> = Object.fromEntries(
  Object.entries(uiCatalog.data.components).map(([type, def]) => [
    type,
    def.props as ZodTypeAny,
  ]),
);

/**
 * Resolve raw message content to a JSON-Render {@link Spec}, or `null` when the
 * content is not a well-formed, catalog-valid spec.
 *
 * @param content Raw message content (accepts `null`/`undefined` defensively).
 */
export function resolveSpec(content: string | null | undefined): Spec | null {
  try {
    if (typeof content !== "string") {
      return null;
    }

    const trimmed = content.trim();
    if (trimmed.length === 0) {
      return null;
    }

    const candidate = extractJsonObject(trimmed);
    if (candidate === null) {
      return null;
    }

    if (!looksLikeSpec(candidate)) {
      return null;
    }

    const result = uiCatalog.validate(candidate);
    if (!result.success || result.data === undefined) {
      return null;
    }

    const spec = result.data as Spec;

    // The multi-component catalog `validate` path treats `props` as an opaque
    // record, so enforce each element's declared prop schema here: reject on any
    // invalid props and strip undeclared keys from the resolved data.
    if (!validateElementProps(spec)) {
      return null;
    }

    // Structural prop validation passes even when the tree's references are
    // broken (a `root` naming no element, or a `children` entry naming no
    // element). `validateSpec` covers exactly that reference integrity; a
    // dangling reference would render blank, so we fall back instead of
    // forwarding such a spec.
    const integrity = validateSpec(spec);
    if (!integrity.valid) {
      return null;
    }

    return spec;
  } catch {
    // Defensive: nothing in this path should throw, but a spec resolver must
    // never break the message-rendering pipeline.
    return null;
  }
}

/** The pipeline stage at which {@link explainSpec} rejected a candidate. */
export type SpecDiagnosticStage =
  | "empty"
  | "json"
  | "shape"
  | "components"
  | "props"
  | "references";

/**
 * Result of {@link explainSpec}: either the normalised, renderable spec, or the
 * stage that rejected it plus a human-readable reason.
 */
export type SpecDiagnostic =
  | { ok: true; spec: Spec }
  | { ok: false; stage: SpecDiagnosticStage; message: string };

/**
 * Authoring / playground counterpart to {@link resolveSpec}: run the *same*
 * validation pipeline but, instead of collapsing every failure to `null`,
 * report which stage rejected the spec and why. Used by the components
 * playground so an author editing raw JSON sees the concrete reason — syntax
 * error, missing fields, unknown component, bad prop, dangling reference —
 * rather than a blank pane.
 *
 * On success the returned `spec` is normalised exactly as `resolveSpec`
 * normalises it (undeclared props stripped), so the caller renders it through
 * the same `<Renderer>` the chat uses. Accepts a bare JSON object or a single
 * ```json fenced block (mirroring `resolveSpec`'s carriers); the playground
 * feeds raw JSON, so a bare object is the common case.
 *
 * @param content Raw spec text (accepts `null`/`undefined` defensively).
 */
export function explainSpec(content: string | null | undefined): SpecDiagnostic {
  if (typeof content !== "string" || content.trim().length === 0) {
    return { ok: false, stage: "empty", message: "请输入一个 JSON spec。" };
  }

  const trimmed = content.trim();
  const jsonText = extractFencedJson(trimmed) ?? trimmed;

  let parsed: unknown;
  try {
    parsed = JSON.parse(jsonText);
  } catch (error) {
    return {
      ok: false,
      stage: "json",
      message: `JSON 语法错误：${(error as Error).message}`,
    };
  }

  if (!isPlainObject(parsed)) {
    return { ok: false, stage: "shape", message: "顶层必须是一个 JSON 对象。" };
  }
  if (!looksLikeSpec(parsed)) {
    return {
      ok: false,
      stage: "shape",
      message: '缺少必需字段：顶层需要 { "root": string, "elements": object }。',
    };
  }

  const result = uiCatalog.validate(parsed);
  if (!result.success || result.data === undefined) {
    return {
      ok: false,
      stage: "components",
      message: formatZodError(result.error),
    };
  }
  const spec = result.data as Spec;

  const propsIssue = firstPropsIssue(spec);
  if (propsIssue !== null) {
    return { ok: false, stage: "props", message: propsIssue };
  }

  const integrity = validateSpec(spec);
  if (!integrity.valid) {
    return {
      ok: false,
      stage: "references",
      message: formatSpecIssues(integrity.issues),
    };
  }

  return { ok: true, spec };
}

/**
 * Cheap heuristic: does this *in-progress* streaming content look like a
 * JSON-Render spec that is still being generated?
 *
 * During streaming the accumulated content is almost always an unclosed JSON
 * fragment, so {@link resolveSpec} cannot decide (and returns `null`). This
 * function answers a narrower, parse-free question used solely to swap in a
 * loading placeholder while a spec streams in — rather than rendering half a
 * JSON blob character by character. It keys off the spec's `"root"` +
 * `"elements"` markers.
 *
 * It is intentionally precise to avoid misfiring on ordinary prose replies: the
 * trimmed content (after an optional opening ```fence) must start with `{` and
 * contain BOTH the `"root"` and `"elements"` markers. A normal streamed message
 * (plain text, or JSON config without those markers) fails at least one check
 * and is left to the existing markdown /
 * envelope-placeholder paths.
 *
 * @param content Raw, possibly-partial streaming content (accepts
 *   `null`/`undefined` defensively).
 */
export function looksLikeStreamingSpec(
  content: string | null | undefined,
): boolean {
  if (!content) {
    return false;
  }
  const trimmed = content.trimStart();
  const body = trimmed.startsWith("```")
    ? trimmed.replace(/^```[a-zA-Z0-9]*\s*/, "")
    : trimmed;
  return (
    body.startsWith("{") &&
    body.includes('"root"') &&
    body.includes('"elements"')
  );
}

/**
 * Validate (and normalise) every element's `props` against its component's
 * catalog Zod schema — the boolean hot-path form used by {@link resolveSpec}.
 * Delegates to {@link firstPropsIssue}, which performs the mutation (stripping
 * undeclared keys) and reports the first failure; here we only need pass/fail.
 */
function validateElementProps(spec: Spec): boolean {
  return firstPropsIssue(spec) === null;
}

/**
 * Walk every element, parsing its `props` through the component's catalog Zod
 * schema and mutating `props` to the parsed result so undeclared keys are
 * stripped from the resolved spec's data. Returns a human-readable message for
 * the first element whose props fail (naming the element key, component type,
 * and offending fields), or `null` when every element passes. An element whose
 * `type` has no schema (defensive: `uiCatalog.validate` already rejected unknown
 * types) is skipped, not rejected.
 */
function firstPropsIssue(spec: Spec): string | null {
  for (const [key, element] of Object.entries(spec.elements) as [
    string,
    UIElement,
  ][]) {
    const schema = componentPropsSchemas[element.type];
    if (schema === undefined) {
      continue;
    }
    const parsed = schema.safeParse(element.props ?? {});
    if (!parsed.success) {
      return `元素 "${key}"（${element.type}）的 props 不合法：${formatZodError(parsed.error)}`;
    }
    element.props = parsed.data as Record<string, unknown>;
  }
  return null;
}

/** Format a Zod error's issues into a compact `path: message；…` string. */
function formatZodError(error: ZodError | undefined): string {
  if (error === undefined || error.issues.length === 0) {
    return "结构校验失败。";
  }
  return error.issues
    .map((issue) => {
      const path = issue.path.join(".");
      return path.length > 0 ? `${path}: ${issue.message}` : issue.message;
    })
    .join("；");
}

/**
 * Extract a parsed JSON object from the trimmed content when the *entire*
 * message is exactly one carrier — a bare JSON object or a single ```json
 * fenced block. Returns the parsed object, or `null` on any other shape or a
 * parse failure. Never throws.
 */
function extractJsonObject(trimmed: string): Record<string, unknown> | null {
  const text = extractBareJson(trimmed) ?? extractFencedJson(trimmed);
  if (text === null) {
    return null;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }

  return isPlainObject(parsed) ? parsed : null;
}

/**
 * Return the trimmed text when it is exactly one JSON object (first char `{`,
 * last char `}`), otherwise `null`. Cheap bracket check only; full validity is
 * decided by `JSON.parse` in the caller.
 */
function extractBareJson(trimmed: string): string | null {
  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    return trimmed;
  }
  return null;
}

/**
 * Return the inner body when the trimmed text is exactly one ```json fenced
 * code block (and nothing else), otherwise `null`. The opening fence's info
 * string's first whitespace-delimited token must equal `json` (case-insensitive);
 * the fence must be closed; nothing but whitespace may follow the close.
 */
function extractFencedJson(trimmed: string): string | null {
  const normalized = trimmed.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");

  const openMatch = /^[ \t]*```(.*)$/.exec(lines[0]);
  if (openMatch === null) {
    return null;
  }

  const langToken = openMatch[1].trim().split(/\s+/, 1)[0] ?? "";
  if (langToken.toLowerCase() !== "json") {
    return null;
  }

  let closeIndex = -1;
  for (let i = 1; i < lines.length; i++) {
    if (/^[ \t]*```\s*$/.test(lines[i])) {
      closeIndex = i;
      break;
    }
  }
  if (closeIndex === -1) {
    return null;
  }

  for (let i = closeIndex + 1; i < lines.length; i++) {
    if (lines[i].trim().length > 0) {
      return null;
    }
  }

  const inner = lines.slice(1, closeIndex).join("\n").trim();
  return inner.length === 0 ? null : inner;
}

/**
 * Structural pre-check before delegating to `uiCatalog.validate`: a spec must
 * carry a string `root` and an object `elements`. This cheaply rejects ordinary
 * JSON objects (e.g. a translation payload) before the heavier catalog pass.
 */
function looksLikeSpec(value: Record<string, unknown>): boolean {
  return typeof value["root"] === "string" && isPlainObject(value["elements"]);
}

/** Narrow to a non-null, non-array object (a JSON object literal). */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
