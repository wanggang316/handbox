/**
 * Validation for the `translation` renderer payload.
 *
 * Pure TypeScript — imports no `.svelte` component — so it is unit-testable in a
 * Node environment. The parser passes `Envelope.data` through verbatim (it never
 * shape-checks the payload), so the input here may be `undefined`, a non-object,
 * an array, etc.; this function is the shape gate.
 *
 * Contract:
 *  - `translation` is required and must be a string that is non-empty after
 *    trimming; otherwise the payload is rejected (`null`).
 *  - Non-object input (`undefined`, `null`, arrays, primitives) is rejected.
 *  - `term`, `phonetic`, and `explanation` are optional: absent keys are fine,
 *    and present-but-non-string values are **dropped** (the returned object
 *    simply omits that key) rather than failing the whole payload.
 *  - Any extra fields are ignored.
 */

import type { TranslationData } from "./types";

/**
 * Validate and normalize a raw `translation` payload into {@link TranslationData},
 * or return `null` when the payload is not renderable.
 *
 * @param data The raw envelope payload (passed through by the parser unchecked).
 */
export function validateTranslation(data: unknown): TranslationData | null {
  if (!isPlainObject(data)) {
    return null;
  }

  const { translation } = data;
  if (typeof translation !== "string" || translation.trim().length === 0) {
    return null;
  }

  const result: TranslationData = { translation };

  // Optional fields: carry through only when they are strings; drop otherwise.
  if (typeof data["term"] === "string") {
    result.term = data["term"];
  }
  if (typeof data["phonetic"] === "string") {
    result.phonetic = data["phonetic"];
  }
  if (typeof data["explanation"] === "string") {
    result.explanation = data["explanation"];
  }

  return result;
}

/**
 * Narrow to a non-null, non-array object.
 */
function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
