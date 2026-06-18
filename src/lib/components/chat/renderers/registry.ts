/**
 * Renderer registry.
 *
 * Maps an envelope `type` (the `__render` discriminator) to the {@link Renderer}
 * that handles it. Matching is **exact and case-sensitive** (`'Translation'` is
 * not `'translation'`). The registry is a thin, dependency-free wrapper around a
 * `Map`, so it stays pure TypeScript and is safe to import from Node-environment
 * unit tests — the concrete renderers (which import `.svelte` components) are
 * assembled and registered in a separate runtime entry.
 */

import type { Renderer } from "./types";

/**
 * A mutable collection of renderers keyed by their `type`.
 *
 * Renderers may carry heterogeneous normalized-data/component types, so entries
 * are stored as `Renderer` with its default (`unknown`) type parameters. Callers
 * narrow at the use site.
 */
export class RendererRegistry {
  private readonly renderers = new Map<string, Renderer>();

  /**
   * Register a renderer under its `type`.
   *
   * Re-registering an already-known `type` overwrites the previous entry
   * (last-writer-wins, via `Map.set`).
   */
  register(renderer: Renderer): void {
    this.renderers.set(renderer.type, renderer);
  }

  /**
   * Look up the renderer for an envelope `type`.
   *
   * Returns `null` — never throws — when the type is unregistered, when the
   * registry is empty, or when `type` is not a usable string (`undefined`,
   * `null`, or empty). Matching is exact and case-sensitive.
   */
  lookup(type: string | null | undefined): Renderer | null {
    if (typeof type !== "string" || type.length === 0) {
      return null;
    }
    return this.renderers.get(type) ?? null;
  }
}

/**
 * The shared registry instance. The runtime entry (`index.ts`) registers the
 * built-in renderers onto it; consumers import this instance and call
 * {@link RendererRegistry.lookup}.
 */
export const rendererRegistry = new RendererRegistry();
