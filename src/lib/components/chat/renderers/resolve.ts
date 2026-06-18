/**
 * Renderer resolution helper for the message-rendering integration.
 *
 * Bridges the pure pipeline (parser + registry + per-renderer validation) and
 * the Svelte view layer: given raw message content, it answers "should this
 * message render as a card, and if so which component with what props?".
 *
 * The flow is `parseEnvelope` -> `rendererRegistry.lookup(type)` ->
 * `renderer.validate(data)`. A miss at any stage (not an envelope, unknown
 * type, or payload that fails validation) yields `null`, signalling the caller
 * to fall back to its existing markdown rendering. Like the layers it composes,
 * this function never throws.
 *
 * The component is exposed as `Component<Record<string, unknown>>` and the
 * validated data as `Record<string, unknown>`, so the caller can spread the
 * normalized payload onto the component as props (e.g. `<Comp {...data} />`)
 * without resorting to `any`.
 */

import type { Component } from "svelte";
import { rendererRegistry } from "./index";
import { parseEnvelope } from "./envelope";

/** A resolved render hit: the component to draw and the props to spread onto it. */
export interface ResolvedRenderer {
  component: Component<Record<string, unknown>>;
  data: Record<string, unknown>;
}

/**
 * Resolve raw message content to a renderable component + props, or `null` when
 * the content is not a renderable envelope.
 *
 * @param content Raw message content (accepts `null`/`undefined` defensively).
 */
export function resolveRenderer(
  content: string | null | undefined,
): ResolvedRenderer | null {
  const envelope = parseEnvelope(content);
  if (envelope === null) {
    return null;
  }

  const renderer = rendererRegistry.lookup(envelope.type);
  if (renderer === null) {
    return null;
  }

  const data = renderer.validate(envelope.data);
  if (data === null || typeof data !== "object") {
    return null;
  }

  return {
    component: renderer.component as Component<Record<string, unknown>>,
    data: data as Record<string, unknown>,
  };
}
