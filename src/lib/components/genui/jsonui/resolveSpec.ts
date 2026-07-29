/**
 * JSON-Render spec resolver.
 *
 * A message qualifies as a renderable {@link Spec} only when the whole trimmed
 * message is exactly one carrier — a bare JSON object, or a single ```json
 * fenced block with nothing but whitespace around it — and the payload has a
 * string `root` plus an object `elements`, passes `uiCatalog.validate`, passes
 * per-component prop validation, and passes `validateSpec` reference integrity.
 * No `__render` discriminator is required.
 *
 * Nothing here throws: every failure yields `null`, signalling the caller to
 * fall back to ordinary markdown rendering.
 */

import type { Spec, UIElement } from "@json-render/core";
import { formatSpecIssues, validateSpec } from "@json-render/core";
import type { ZodError, ZodTypeAny } from "zod";
import { uiCatalog } from "./catalog";

/**
 * Per-component prop schemas keyed by component type. `uiCatalog.validate`
 * checks structure only and leaves `props` an opaque record, so prop shape is
 * enforced (and stripped to) separately via this map.
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

    if (!validateElementProps(spec)) {
      return null;
    }

    // Structural validation passes even with a dangling `root`/`children` id,
    // which would render blank; fall back instead of forwarding such a spec.
    const integrity = validateSpec(spec);
    if (!integrity.valid) {
      return null;
    }

    return spec;
  } catch {
    // A spec resolver must never break the message-rendering pipeline.
    return null;
  }
}

export type SpecDiagnosticStage =
  | "empty"
  | "json"
  | "shape"
  | "components"
  | "props"
  | "references";

export type SpecDiagnostic =
  | { ok: true; spec: Spec }
  | { ok: false; stage: SpecDiagnosticStage; message: string };

/**
 * Authoring counterpart to {@link resolveSpec}: runs the same pipeline but
 * reports the rejecting stage and a human-readable reason instead of collapsing
 * every failure to `null`. On success the spec is normalised identically
 * (undeclared props stripped), so it renders through the same `<Renderer>`.
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
 * Parse-free heuristic for still-streaming content, used solely to swap in a
 * loading placeholder rather than render half a JSON blob. Deliberately strict
 * so it does not misfire on prose: after an optional opening fence the body must
 * start with `{` and contain BOTH the `"root"` and `"elements"` markers.
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

/** Boolean form of {@link firstPropsIssue}, which also strips undeclared props. */
function validateElementProps(spec: Spec): boolean {
  return firstPropsIssue(spec) === null;
}

/**
 * Parse each element's `props` through its component's catalog Zod schema,
 * mutating `props` to the parsed result so undeclared keys are stripped from the
 * resolved spec. Returns a message for the first failing element, or `null` when
 * all pass; an element whose `type` has no schema is skipped, not rejected
 * (`uiCatalog.validate` has already rejected unknown types).
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
 * Parse the trimmed content when the *entire* message is exactly one carrier — a
 * bare JSON object or a single ```json fenced block. `null` on any other shape
 * or on a parse failure; never throws.
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

/** Cheap bracket check only; full validity is decided by `JSON.parse` in the caller. */
function extractBareJson(trimmed: string): string | null {
  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    return trimmed;
  }
  return null;
}

/**
 * Inner body of the trimmed text when it is exactly one ```json fenced block,
 * otherwise `null`: the info string's first whitespace-delimited token must
 * equal `json` (case-insensitive), the fence must be closed, and nothing but
 * whitespace may follow the close.
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
 * Cheap pre-check that rejects ordinary JSON objects before the heavier catalog
 * pass: a spec must carry a string `root` and an object `elements`.
 */
function looksLikeSpec(value: Record<string, unknown>): boolean {
  return typeof value["root"] === "string" && isPlainObject(value["elements"]);
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
