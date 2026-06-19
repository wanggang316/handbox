/**
 * JSON-Render catalog for the generative-UI PoC.
 *
 * Declares the ten presentational components (Card, Text, Badge, Stack,
 * StatusLabel, Avatar, Divider, KeyValue, Table, InfoTooltip) that an AI may
 * compose into a flat {@link Spec}. The catalog drives
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
    StatusLabel: {
      props: z.object({
        status: z.enum(["enabled", "disabled", "idle", "error"]),
        text: z.string(),
      }),
      description:
        "A pill-shaped status label. 'enabled' is green, 'error' is red, 'disabled' is neutral grey, 'idle' is blue.",
    },
    Avatar: {
      props: z.object({
        letter: z.string(),
        size: z.enum(["sm", "md", "lg"]).optional(),
      }),
      description:
        "A circular avatar placeholder showing the uppercased first character of 'letter'. Display-only; no image.",
    },
    Divider: {
      props: z.object({
        orientation: z.enum(["horizontal", "vertical"]).optional(),
      }),
      description:
        "A thin separator line. Defaults to a horizontal rule; use 'vertical' inside a row.",
    },
    KeyValue: {
      props: z.object({
        items: z.array(z.object({ key: z.string(), value: z.string() })),
      }),
      description:
        "A list of key/value rows, one row per item rendered as 'key — value'. An empty list renders nothing.",
    },
    Table: {
      props: z.object({
        columns: z.array(z.string()),
        rows: z.array(z.array(z.string())),
      }),
      description:
        "A read-only table. 'columns' is the header; each entry in 'rows' is one data row of cell strings.",
    },
    InfoTooltip: {
      props: z.object({
        content: z.string(),
      }),
      description:
        "A help icon that reveals 'content' in a hover popover. Use for inline explanatory hints.",
    },
  },
  // The schema's catalog shape types `actions` as required; this PoC exposes no
  // actions, so an empty map satisfies the type without adding behaviour.
  actions: {},
});
