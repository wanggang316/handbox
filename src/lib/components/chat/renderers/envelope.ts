/**
 * Render-envelope parser.
 *
 * Detects whether an agent message carries a render directive and, if so,
 * extracts it as an {@link Envelope}. Two carriers are supported:
 *
 *  1. **Bare JSON** — the entire message (after trimming) is exactly one JSON
 *     object. Any non-whitespace text before or after the object disqualifies it.
 *  2. **A single ```json fenced code block** — the entire message (after
 *     trimming) is exactly one fenced block whose language token is `json`
 *     (case-insensitive). Any prose outside the fence, multiple fences, a
 *     non-`json` language, or an unclosed fence disqualifies it.
 *
 * Structural validation (shared by both carriers): the parsed value must be a
 * plain object carrying a non-empty string `__render` discriminator. The
 * payload (`data`) is passed through verbatim and is NOT shape-checked here —
 * that is the responsibility of the downstream renderer.
 *
 * The function never throws: any malformed, empty, or non-envelope input
 * yields `null`.
 */

import type { Envelope } from "./types";

/**
 * Parse a message's content into a render {@link Envelope}, or `null` when the
 * content is not a well-formed envelope.
 *
 * @param content Raw message content. Accepts `null`/`undefined` defensively.
 */
export function parseEnvelope(content: string | null | undefined): Envelope | null {
  if (typeof content !== "string") {
    return null;
  }

  const trimmed = content.trim();
  if (trimmed.length === 0) {
    return null;
  }

  // The message must be *exactly* one carrier — either a bare JSON object or a
  // single ```json fenced block — with nothing else around it.
  const candidate = extractBareJson(trimmed) ?? extractFencedJson(trimmed);
  if (candidate === null) {
    return null;
  }

  const parsed = tryParseJson(candidate);
  if (parsed === undefined) {
    return null;
  }

  return toEnvelope(parsed);
}

/**
 * Cheap heuristic: does this *in-progress* streaming content look like a render
 * envelope that is still being generated?
 *
 * During streaming the accumulated content is almost always an unclosed JSON
 * fragment, so {@link parseEnvelope} cannot decide. This function answers a
 * narrower, parse-free question used solely to swap in a loading placeholder
 * while a render envelope streams in (rather than rendering half a JSON blob
 * character by character).
 *
 * It is intentionally precise to avoid misfiring on ordinary prose replies:
 * BOTH conditions must hold on the trimmed content —
 *
 *  1. it starts with a render-envelope carrier opener: a bare `{`, OR a
 *     ```json fence whose language token is `json` (case-insensitive); AND
 *  2. it contains the `"__render"` discriminator marker somewhere.
 *
 * A normal streamed message (plain text, or JSON without `__render`, or text
 * that does not open with `{`/```json) fails at least one check and is left to
 * the existing per-character markdown rendering path.
 *
 * @param content Raw, possibly-partial streaming content (accepts
 *   `null`/`undefined` defensively).
 */
export function looksLikeStreamingEnvelope(
  content: string | null | undefined,
): boolean {
  if (typeof content !== "string") {
    return false;
  }

  const trimmed = content.trim();
  if (trimmed.length === 0) {
    return false;
  }

  if (!startsWithEnvelopeOpener(trimmed)) {
    return false;
  }

  // The `__render` discriminator is what distinguishes a render envelope from
  // any other JSON object. Match the quoted key so plain prose mentioning the
  // word "render" cannot trip the heuristic.
  return trimmed.includes('"__render"');
}

/**
 * Whether the trimmed content opens with a render-envelope carrier: a bare JSON
 * object (`{`) or a ```json fenced code block whose language token is `json`
 * (case-insensitive). The fence may be indented and may carry a trailing info
 * string (first whitespace-delimited token wins).
 */
function startsWithEnvelopeOpener(trimmed: string): boolean {
  if (trimmed.startsWith("{")) {
    return true;
  }

  const firstLine = trimmed.split(/\r\n?|\n/, 1)[0] ?? "";
  const openMatch = /^[ \t]*```(.*)$/.exec(firstLine);
  if (openMatch === null) {
    return false;
  }

  const langToken = openMatch[1].trim().split(/\s+/, 1)[0] ?? "";
  return langToken.toLowerCase() === "json";
}

/**
 * Return the trimmed text when it is exactly one JSON object (first char `{`,
 * last char `}`), otherwise `null`. We only do a cheap bracket check here;
 * full validity is decided by `JSON.parse` later. This deliberately rejects
 * "prefix text + object" because the whole message must be the object.
 */
function extractBareJson(trimmed: string): string | null {
  if (trimmed.startsWith("{") && trimmed.endsWith("}")) {
    return trimmed;
  }
  return null;
}

/**
 * Return the inner body when the trimmed text is exactly one ```json fenced
 * code block (and nothing else), otherwise `null`.
 *
 * Recognition rules:
 *  - The block must start with an opening fence line (``` optionally indented)
 *    whose info string's first whitespace-delimited token equals `json`
 *    (case-insensitive). `json5`/`jsonc`/no-lang/other languages are rejected;
 *    a trailing info string like `json title=x` is accepted (first token wins).
 *  - The block must have a closing fence; an unclosed fence is rejected.
 *  - Nothing but whitespace may follow the closing fence (no outside prose,
 *    no second fenced block).
 */
function extractFencedJson(trimmed: string): string | null {
  // Normalize CRLF / lone CR so line handling is uniform.
  const normalized = trimmed.replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");

  const opening = lines[0];
  const openMatch = /^[ \t]*```(.*)$/.exec(opening);
  if (openMatch === null) {
    return null;
  }

  const infoString = openMatch[1].trim();
  const langToken = infoString.split(/\s+/, 1)[0] ?? "";
  if (langToken.toLowerCase() !== "json") {
    return null;
  }

  // Find the closing fence among the remaining lines.
  let closeIndex = -1;
  for (let i = 1; i < lines.length; i++) {
    if (/^[ \t]*```\s*$/.test(lines[i])) {
      closeIndex = i;
      break;
    }
  }
  if (closeIndex === -1) {
    // Unclosed fence.
    return null;
  }

  // Anything after the closing fence (besides whitespace) means the message
  // is not exactly one fenced block.
  for (let i = closeIndex + 1; i < lines.length; i++) {
    if (lines[i].trim().length > 0) {
      return null;
    }
  }

  const inner = lines.slice(1, closeIndex).join("\n").trim();
  if (inner.length === 0) {
    return null;
  }
  return inner;
}

/**
 * Parse JSON, returning the parsed value or `undefined` on any parse error.
 * `undefined` is used as the failure sentinel because `JSON.parse("null")`
 * legitimately yields `null`.
 */
function tryParseJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/**
 * Apply the shared structural contract to a parsed JSON value: it must be a
 * plain object carrying a non-empty string `__render`. Returns the envelope or
 * `null`.
 */
function toEnvelope(value: unknown): Envelope | null {
  if (!isPlainObject(value)) {
    return null;
  }

  const render = value["__render"];
  if (typeof render !== "string" || render.trim().length === 0) {
    return null;
  }

  return {
    type: render,
    // Pass the payload through verbatim — no shape validation at this layer.
    data: value["data"],
  };
}

/**
 * Narrow to a non-null, non-array object (a JSON object literal).
 */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
