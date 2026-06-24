/**
 * Unit tests for the JSON-Render {@link uiCatalog} — focused on the six
 * presentational catalog components added by the catalog-expansion milestone
 * (StatusLabel / Avatar / Divider / KeyValue / Table / InfoTooltip).
 *
 * Pure TypeScript — imports `catalog.ts` only (no `.svelte`), so the suite runs
 * under the plain-Node Vitest environment.
 *
 * Scope note: `uiCatalog.validate` is the catalog's *structural* pass. With more
 * than one component registered, `@json-render/core` types each element's `props`
 * as an opaque record (`z.record(z.string(), z.unknown())`) and does NOT apply
 * the per-component Zod prop schema. So `validate` here asserts only structure:
 * spec shape, `type` membership, and the `children`/`visible` element fields.
 * Per-component prop validation (required / type / enum / extra-prop strip) is
 * enforced by `resolveSpec` and is covered in `resolveSpec.test.ts`.
 *
 * This test also pins that structural behaviour so that the prop-enforcement
 * responsibility staying in `resolveSpec` is intentional, not accidental.
 *
 * The generated catalog validator requires every element to carry both
 * `children` (array) and `visible`, so the single-element fixtures below include
 * them on each element.
 */

import { describe, it, expect } from "vitest";
import { uiCatalog } from "./catalog";

/** Wrap a single element into a minimal one-node spec rooted at it. */
function specOf(element: Record<string, unknown>) {
  return { root: "x", elements: { x: element } };
}

/** Minimal valid single-element specs for each new component type. */
const minimalElements = {
  StatusLabel: {
    type: "StatusLabel",
    props: { status: "enabled", text: "Online" },
    children: [],
    visible: true,
  },
  Avatar: {
    type: "Avatar",
    props: { letter: "alice", size: "md" },
    children: [],
    visible: true,
  },
  Divider: {
    type: "Divider",
    props: { orientation: "horizontal" },
    children: [],
    visible: true,
  },
  KeyValue: {
    type: "KeyValue",
    props: { items: [{ key: "Name", value: "Ada" }] },
    children: [],
    visible: true,
  },
  Table: {
    type: "Table",
    props: { columns: ["A", "B"], rows: [["1", "2"]] },
    children: [],
    visible: true,
  },
  InfoTooltip: {
    type: "InfoTooltip",
    props: { content: "Helpful hint" },
    children: [],
    visible: true,
  },
} as const;

describe("uiCatalog — registers the six expansion components", () => {
  it("exposes all six new component names", () => {
    for (const name of [
      "StatusLabel",
      "Avatar",
      "Divider",
      "KeyValue",
      "Table",
      "InfoTooltip",
    ]) {
      expect(uiCatalog.componentNames).toContain(name);
    }
  });

  it("attaches a non-empty description to each new component (for the LLM prompt)", () => {
    const components = uiCatalog.data.components as Record<
      string,
      { description?: string }
    >;
    for (const name of [
      "StatusLabel",
      "Avatar",
      "Divider",
      "KeyValue",
      "Table",
      "InfoTooltip",
    ]) {
      expect(components[name]?.description?.length ?? 0).toBeGreaterThan(0);
    }
  });
});

describe("uiCatalog.validate — structural validation per new component", () => {
  for (const [name, element] of Object.entries(minimalElements)) {
    it(`validates a minimal ${name} spec`, () => {
      const result = uiCatalog.validate(specOf(element));
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
    });
  }

  it("validates KeyValue with an empty items array and Table with zero rows", () => {
    expect(
      uiCatalog.validate(
        specOf({
          type: "KeyValue",
          props: { items: [] },
          children: [],
          visible: true,
        }),
      ).success,
    ).toBe(true);
    expect(
      uiCatalog.validate(
        specOf({
          type: "Table",
          props: { columns: ["A", "B"], rows: [] },
          children: [],
          visible: true,
        }),
      ).success,
    ).toBe(true);
  });
});

describe("uiCatalog.validate — element shape requirements", () => {
  it("rejects an element missing `children`", () => {
    expect(
      uiCatalog.validate(
        specOf({
          type: "Divider",
          props: {},
          visible: true,
        }),
      ).success,
    ).toBe(false);
  });

  it("rejects an element missing `visible`", () => {
    expect(
      uiCatalog.validate(
        specOf({
          type: "Divider",
          props: {},
          children: [],
        }),
      ).success,
    ).toBe(false);
  });
});

describe("uiCatalog.validate — unregistered / case-sensitive types are rejected", () => {
  it("rejects an unknown component type", () => {
    expect(
      uiCatalog.validate(
        specOf({
          type: "Carousel",
          props: {},
          children: [],
          visible: true,
        }),
      ).success,
    ).toBe(false);
  });

  it("rejects a case-mismatched type (`statuslabel`)", () => {
    expect(
      uiCatalog.validate(
        specOf({
          type: "statuslabel",
          props: { status: "enabled", text: "x" },
          children: [],
          visible: true,
        }),
      ).success,
    ).toBe(false);
  });
});
