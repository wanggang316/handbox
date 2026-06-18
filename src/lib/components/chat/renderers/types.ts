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
