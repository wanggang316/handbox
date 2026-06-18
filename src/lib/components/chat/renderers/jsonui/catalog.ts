/**
 * JSON-Render catalog for the generative-UI PoC.
 *
 * Declares the four minimal presentational components (Card / Text / Badge /
 * Stack) that an AI may compose into a flat {@link Spec}. The catalog drives
 * three things:
 *
 *  - `uiCatalog.validate(spec)` — structural + per-component prop validation,
 *    used by {@link resolveSpec} to decide whether a message is a renderable spec.
 *  - `uiCatalog.prompt()` — the system-prompt fragment handed to the LLM so it
 *    knows the available components and their props.
 *  - the typed component set consumed by `registry.ts` (which binds `.svelte`).
 *
 * This module imports only the schema + Zod (no `.svelte`), so it is safe to
 * pull into the Node-environment unit tests.
 */

import { defineCatalog } from "@json-render/core";
// Import the Svelte schema from its dedicated subpath rather than the package
// root: the root entry (`@json-render/svelte`) re-exports `.svelte` modules,
// which the plain-Node Vitest environment (no SvelteKit plugin) cannot load.
// `@json-render/svelte/schema` is pure JS and exports the identical `schema`.
import { schema } from "@json-render/svelte/schema";
import { z } from "zod";

export const uiCatalog = defineCatalog(schema, {
  components: {
    Card: {
      props: z.object({ title: z.string().optional() }),
      slots: ["default"],
      description: "A bordered card container. Put content in children.",
    },
    Text: {
      props: z.object({
        text: z.string(),
        variant: z.enum(["body", "heading", "muted"]).optional(),
      }),
      description:
        "A text block. Use variant 'heading' for titles, 'muted' for secondary text.",
    },
    Badge: {
      props: z.object({
        label: z.string(),
        tone: z.enum(["info", "success", "warning", "error"]).optional(),
      }),
      description: "A small status badge.",
    },
    Stack: {
      props: z.object({
        gap: z.enum(["sm", "md", "lg"]).optional(),
        direction: z.enum(["row", "col"]).optional(),
      }),
      slots: ["default"],
      description: "A flex layout container. Put items in children.",
    },
  },
  // The schema's catalog shape types `actions` as required; this PoC exposes no
  // actions, so an empty map satisfies the type without adding behaviour.
  actions: {},
});
