/**
 * Unit tests for {@link resolveSpec}.
 *
 * Pure TypeScript — imports `resolveSpec.ts` (and transitively `catalog.ts`),
 * neither of which pulls in a `.svelte` module, so the suite runs under the
 * plain-Node Vitest environment. The runtime `registry.ts` (which imports
 * `.svelte`) is intentionally NOT imported here.
 *
 * The catalog validator requires every element to carry both `children` (array)
 * and `visible` — a quirk of the generated Zod schema — so the fixtures include
 * them on each element.
 */

import { describe, it, expect } from "vitest";
import { resolveSpec } from "./resolveSpec";

/** A catalog-valid spec: a Card → Stack → (Text, Badge) tree. */
const validSpec = {
  root: "card",
  elements: {
    card: {
      type: "Card",
      props: { title: "serendipity" },
      children: ["stack"],
      visible: true,
    },
    stack: {
      type: "Stack",
      props: { gap: "md" },
      children: ["title", "pos"],
      visible: true,
    },
    title: {
      type: "Text",
      props: { text: "意外发现珍宝的运气", variant: "heading" },
      children: [],
      visible: true,
    },
    pos: {
      type: "Badge",
      props: { label: "n. 名词", tone: "info" },
      children: [],
      visible: true,
    },
  },
};

describe("resolveSpec", () => {
  it("returns a bare-JSON spec that validates against the catalog", () => {
    const result = resolveSpec(JSON.stringify(validSpec));
    expect(result).not.toBeNull();
    expect(result?.root).toBe("card");
    expect(Object.keys(result?.elements ?? {})).toContain("stack");
  });

  it("returns a spec wrapped in a single ```json fenced block", () => {
    const fenced = "```json\n" + JSON.stringify(validSpec, null, 2) + "\n```";
    const result = resolveSpec(fenced);
    expect(result).not.toBeNull();
    expect(result?.root).toBe("card");
  });

  it("returns null for a spec referencing an unregistered component type", () => {
    const unknownType = {
      root: "x",
      elements: {
        x: { type: "Carousel", props: {}, children: [], visible: true },
      },
    };
    expect(resolveSpec(JSON.stringify(unknownType))).toBeNull();
  });

  it("returns null for a well-formed JSON object that is not a spec", () => {
    // A render-envelope-style payload: valid JSON, but no root/elements shape.
    const notSpec = { __render: "translation", data: { term: "serendipity" } };
    expect(resolveSpec(JSON.stringify(notSpec))).toBeNull();
  });

  it("returns null for a spec missing the string `root` field", () => {
    const noRoot = {
      elements: {
        x: { type: "Text", props: { text: "hi" }, children: [], visible: true },
      },
    };
    expect(resolveSpec(JSON.stringify(noRoot))).toBeNull();
  });

  it("returns null when `elements` is not an object", () => {
    const badElements = { root: "x", elements: [] };
    expect(resolveSpec(JSON.stringify(badElements))).toBeNull();
  });

  it("returns null for malformed JSON", () => {
    expect(resolveSpec('{ "root": "card", elements: }')).toBeNull();
    expect(resolveSpec("{ not json at all")).toBeNull();
  });

  it("returns null for ordinary markdown prose", () => {
    expect(
      resolveSpec("Here is the translation of **serendipity**: 意外发现珍宝的运气."),
    ).toBeNull();
  });

  it("returns null when prose surrounds an otherwise-valid bare spec", () => {
    // The whole message must be exactly one carrier; leading prose disqualifies it.
    const withPrefix = "Sure, here you go:\n" + JSON.stringify(validSpec);
    expect(resolveSpec(withPrefix)).toBeNull();
  });

  it("returns null for an unclosed ```json fence", () => {
    const unclosed = "```json\n" + JSON.stringify(validSpec);
    expect(resolveSpec(unclosed)).toBeNull();
  });

  it("returns null for a non-json fenced block carrying spec JSON", () => {
    const wrongLang = "```ts\n" + JSON.stringify(validSpec) + "\n```";
    expect(resolveSpec(wrongLang)).toBeNull();
  });

  it("returns null for null, undefined, empty, and whitespace input", () => {
    expect(resolveSpec(null)).toBeNull();
    expect(resolveSpec(undefined)).toBeNull();
    expect(resolveSpec("")).toBeNull();
    expect(resolveSpec("   \n  ")).toBeNull();
  });

  it("returns null for a non-object JSON value (array / scalar)", () => {
    expect(resolveSpec("[1, 2, 3]")).toBeNull();
    expect(resolveSpec('"just a string"')).toBeNull();
    expect(resolveSpec("42")).toBeNull();
  });
});
