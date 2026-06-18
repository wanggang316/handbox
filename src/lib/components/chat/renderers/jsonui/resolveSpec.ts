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
 * candidate qualifies as a spec only when it (a) structurally looks like a spec
 * — a string `root` plus an object `elements` — and (b) passes
 * `uiCatalog.validate`, which checks every element's `type`/`props` against the
 * catalog. A spec referencing an unregistered component type therefore fails
 * validation and yields `null`.
 *
 * The function never throws: any malformed, non-spec, or non-JSON input — and
 * any internal validation error — yields `null`, signalling the caller to fall
 * back to ordinary markdown rendering.
 */

import type { Spec } from "@json-render/core";
import { uiCatalog } from "./catalog";

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

    return result.data as Spec;
  } catch {
    // Defensive: nothing in this path should throw, but a spec resolver must
    // never break the message-rendering pipeline.
    return null;
  }
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
