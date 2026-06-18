/**
 * Shared types for the agent-output dynamic renderer pipeline.
 *
 * An "envelope" is a render directive an agent embeds in its message content.
 * The structural contract is: a top-level JSON object carrying a non-empty
 * string `__render` discriminator plus an optional `data` payload. The parser
 * only validates the envelope shell; payload (`data`) shape validation is left
 * to the individual renderers downstream.
 */

/**
 * A successfully parsed render envelope.
 *
 * `type` is the value of the `__render` discriminator (guaranteed non-empty).
 * `data` is the raw payload, passed through untouched (may be `undefined`,
 * a non-object, etc.) — the parser never inspects or coerces it.
 */
export interface Envelope {
  type: string;
  data: unknown;
}

/**
 * Payload for the `translation` renderer.
 *
 * `translation` is the only required field. `term` (the source word/phrase),
 * `phonetic`, and `explanation` are optional — when absent or empty the
 * renderer omits the corresponding row entirely (no empty shells). All fields
 * are plain text and are rendered via Svelte text binding (never `@html`);
 * `explanation` is plain text too (markdown markers are shown verbatim), with
 * line breaks preserved for readability.
 */
export interface TranslationData {
  term?: string;
  translation: string;
  phonetic?: string;
  explanation?: string;
}

/**
 * A renderer plugs a single envelope `type` into the registry.
 *
 * `C` is the component type. It is generic so that the runtime entry can bind a
 * concrete Svelte `Component` (e.g. `TranslationCard`) while tests can register
 * a lightweight placeholder — both without resorting to `any`. The registry
 * itself never inspects `component`; it only stores and returns it.
 *
 * @typeParam N The normalized data shape this renderer produces from `validate`.
 * @typeParam C The component used to render `N`.
 */
export interface Renderer<N = unknown, C = unknown> {
  /** The envelope discriminator this renderer handles (matched exactly, case-sensitive). */
  type: string;
  /**
   * Validate and normalize a raw envelope payload.
   *
   * Receives `Envelope.data` verbatim (the parser does not shape-check it, so
   * this may be `undefined`, a non-object, etc.). Returns the normalized data
   * on success, or `null` when the payload is not renderable by this renderer.
   */
  validate(data: unknown): N | null;
  /** The component used to render the normalized data. */
  component: C;
}
