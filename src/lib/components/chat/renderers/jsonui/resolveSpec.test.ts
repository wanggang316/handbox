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
import { resolveSpec, looksLikeStreamingSpec, explainSpec } from "./resolveSpec";

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

/**
 * A catalog-valid spec exercising the catalog-expansion components: a Card →
 * Stack holding a StatusLabel, an Avatar, a Divider, a KeyValue list, a read-only
 * Table, and an InfoTooltip.
 */
const mixedSpec = {
  root: "card",
  elements: {
    card: {
      type: "Card",
      props: { title: "Profile" },
      children: ["stack"],
      visible: true,
    },
    stack: {
      type: "Stack",
      props: { gap: "md" },
      children: ["status", "avatar", "rule", "kv", "table", "help"],
      visible: true,
    },
    status: {
      type: "StatusLabel",
      props: { status: "enabled", text: "Online" },
      children: [],
      visible: true,
    },
    avatar: {
      type: "Avatar",
      props: { letter: "ada", size: "lg" },
      children: [],
      visible: true,
    },
    rule: {
      type: "Divider",
      props: { orientation: "horizontal" },
      children: [],
      visible: true,
    },
    kv: {
      type: "KeyValue",
      props: { items: [{ key: "Role", value: "Admin" }] },
      children: [],
      visible: true,
    },
    table: {
      type: "Table",
      props: { columns: ["Metric", "Value"], rows: [["Sessions", "12"]] },
      children: [],
      visible: true,
    },
    help: {
      type: "InfoTooltip",
      props: { content: "Status reflects the last heartbeat." },
      children: [],
      visible: true,
    },
  },
};

describe("resolveSpec", () => {
  it("resolves a mixed spec using the catalog-expansion components (bare JSON)", () => {
    const result = resolveSpec(JSON.stringify(mixedSpec));
    expect(result).not.toBeNull();
    expect(result?.root).toBe("card");
    expect(result?.elements?.["status"]?.type).toBe("StatusLabel");
    expect(result?.elements?.["table"]?.type).toBe("Table");
  });

  it("resolves the same mixed spec wrapped in a ```json fenced block", () => {
    const fenced = "```json\n" + JSON.stringify(mixedSpec, null, 2) + "\n```";
    const result = resolveSpec(fenced);
    expect(result).not.toBeNull();
    expect(result?.elements?.["help"]?.type).toBe("InfoTooltip");
  });

  it("returns null for a case-mismatched new-component type", () => {
    const caseMismatch = {
      root: "x",
      elements: {
        x: {
          type: "statuslabel",
          props: { status: "enabled", text: "x" },
          children: [],
          visible: true,
        },
      },
    };
    expect(resolveSpec(JSON.stringify(caseMismatch))).toBeNull();
  });

  it("returns null for a spec whose root names no element (dangling root)", () => {
    const danglingRoot = {
      root: "missing",
      elements: {
        present: {
          type: "Divider",
          props: {},
          children: [],
          visible: true,
        },
      },
    };
    expect(resolveSpec(JSON.stringify(danglingRoot))).toBeNull();
  });

  it("returns null for a spec with a child referencing no element (dangling child)", () => {
    const danglingChild = {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "x" },
          children: ["ghost"],
          visible: true,
        },
      },
    };
    expect(resolveSpec(JSON.stringify(danglingChild))).toBeNull();
  });

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
      resolveSpec(
        "Here is the translation of **serendipity**: 意外发现珍宝的运气.",
      ),
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

/**
 * Per-component prop enforcement happens in `resolveSpec` (not in
 * `uiCatalog.validate`, which leaves multi-component props opaque). These cases
 * pin that boundary: invalid props sink the whole spec to `null`, while
 * undeclared props are stripped from the resolved data.
 */
describe("resolveSpec — per-component prop validation", () => {
  /** Helper: a one-element spec rooted at `x` for the given element. */
  function specOf(element: Record<string, unknown>): string {
    return JSON.stringify({ root: "x", elements: { x: element } });
  }

  it("returns null when a required prop is missing (StatusLabel without text)", () => {
    expect(
      resolveSpec(
        specOf({
          type: "StatusLabel",
          props: { status: "enabled" },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null when a required prop is missing (Avatar without letter)", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Avatar",
          props: { size: "sm" },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null for a wrong prop type (Table columns given a string)", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Table",
          props: { columns: "A,B", rows: [] },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null for a wrong nested prop type (KeyValue item value is a number)", () => {
    expect(
      resolveSpec(
        specOf({
          type: "KeyValue",
          props: { items: [{ key: "n", value: 1 }] },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null for an illegal StatusLabel.status enum value", () => {
    expect(
      resolveSpec(
        specOf({
          type: "StatusLabel",
          props: { status: "online", text: "x" },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null for an illegal Avatar.size enum value", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Avatar",
          props: { letter: "x", size: "xl" },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null for an illegal Divider.orientation enum value", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Divider",
          props: { orientation: "diagonal" },
          children: [],
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null when an element is missing `children`", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Divider",
          props: {},
          visible: true,
        }),
      ),
    ).toBeNull();
  });

  it("returns null when an element is missing `visible`", () => {
    expect(
      resolveSpec(
        specOf({
          type: "Divider",
          props: {},
          children: [],
        }),
      ),
    ).toBeNull();
  });

  it("strips undeclared props so they do not survive into the resolved data", () => {
    const result = resolveSpec(
      specOf({
        type: "StatusLabel",
        props: { status: "enabled", text: "x", rogue: "drop-me" },
        children: [],
        visible: true,
      }),
    );
    expect(result).not.toBeNull();
    const props = result?.elements?.["x"]?.props ?? {};
    expect(props).not.toHaveProperty("rogue");
    expect(props).toMatchObject({ status: "enabled", text: "x" });
  });

  it("strips undeclared optional props while keeping declared optional props", () => {
    const result = resolveSpec(
      specOf({
        type: "Avatar",
        props: { letter: "ada", size: "lg", color: "red" },
        children: [],
        visible: true,
      }),
    );
    expect(result).not.toBeNull();
    const props = result?.elements?.["x"]?.props ?? {};
    expect(props).not.toHaveProperty("color");
    expect(props).toMatchObject({ letter: "ada", size: "lg" });
  });
});

/**
 * `looksLikeStreamingSpec` is the parse-free streaming heuristic that decides
 * whether an in-progress (usually unclosed) message looks like a JSON-Render
 * spec, so the chat bubble can show a loading placeholder instead of rendering
 * a half-finished JSON blob. It must fire on partial spec fragments yet stay
 * silent on prose, plain config JSON, and any other JSON that lacks the spec's
 * `root`/`elements` markers. Boundaries pinned here trace to VAL-STREAM-003.
 */
describe("looksLikeStreamingSpec", () => {
  it("is TRUE for a partial, unclosed spec fragment (bare JSON)", () => {
    const partial = '{ "root": "card", "elements": { "card": { "type":';
    expect(looksLikeStreamingSpec(partial)).toBe(true);
  });

  it("is TRUE for the ```json-fenced partial variant", () => {
    const partial = '```json\n{ "root": "card", "elements": { "card": {';
    expect(looksLikeStreamingSpec(partial)).toBe(true);
  });

  it("is TRUE for a bare ``` fence (no language) carrying a partial spec", () => {
    // Known, intentional edge: the heuristic fires on a bare fence, but
    // resolveSpec/extractFencedJson only accept a ```json tag — so a bare-fence
    // spec degrades gracefully (placeholder → markdown), it is not a bug.
    const partial = '```\n{ "root": "card", "elements": {';
    expect(looksLikeStreamingSpec(partial)).toBe(true);
  });

  it("is FALSE when only `root` is present (no `elements` yet)", () => {
    const partial = '{ "root": "card", "title":';
    expect(looksLikeStreamingSpec(partial)).toBe(false);
  });

  it("is FALSE for plain config JSON carrying neither marker", () => {
    const config = '{ "model": "gpt-4", "temperature": 0.7';
    expect(looksLikeStreamingSpec(config)).toBe(false);
  });

  it("is FALSE for JSON that lacks the spec markers (e.g. a `__render`-style payload)", () => {
    const other = '{ "__render": "translation", "data": { "term":';
    expect(looksLikeStreamingSpec(other)).toBe(false);
  });

  it("is FALSE for ordinary markdown prose", () => {
    expect(
      looksLikeStreamingSpec("Here is the **root** of the elements I found"),
    ).toBe(false);
  });

  it("is FALSE for prose that precedes an otherwise-spec-like object", () => {
    // Must open with `{` (after an optional fence); leading prose disqualifies.
    expect(
      looksLikeStreamingSpec('Sure: { "root": "card", "elements": {'),
    ).toBe(false);
  });

  it("is FALSE for empty, whitespace, null, and undefined input", () => {
    expect(looksLikeStreamingSpec("")).toBe(false);
    expect(looksLikeStreamingSpec("   \n  ")).toBe(false);
    expect(looksLikeStreamingSpec(null)).toBe(false);
    expect(looksLikeStreamingSpec(undefined)).toBe(false);
  });
});

/**
 * The streaming render decision in `MessageAssistant.svelte` is driven by the
 * pair (`resolveSpec`, `looksLikeStreamingSpec`): while the streamed JSON is
 * unclosed, `resolveSpec` is null and the heuristic is true → placeholder; once
 * the spec closes and validates, `resolveSpec` hits → render; a closed spec
 * with an unregistered component fails `resolveSpec` → markdown fallback. This
 * suite pins that transition (VAL-STREAM-001 logic) on the pure functions.
 */
describe("specHit transition (resolveSpec × looksLikeStreamingSpec)", () => {
  it("partial spec → resolveSpec null AND heuristic true (⇒ placeholder)", () => {
    const partial = JSON.stringify(validSpec).slice(0, 60);
    expect(resolveSpec(partial)).toBeNull();
    expect(looksLikeStreamingSpec(partial)).toBe(true);
  });

  it("closed valid spec → resolveSpec non-null (⇒ renders)", () => {
    const closed = JSON.stringify(validSpec);
    expect(resolveSpec(closed)).not.toBeNull();
  });

  it("closed spec with an unregistered component → resolveSpec null (⇒ markdown)", () => {
    const unknownType = JSON.stringify({
      root: "x",
      elements: {
        x: { type: "Carousel", props: {}, children: [], visible: true },
      },
    });
    expect(resolveSpec(unknownType)).toBeNull();
  });
});

describe("explainSpec", () => {
  /** Assert explainSpec rejected the input and return the narrowed failure. */
  function failure(input: string | null | undefined) {
    const result = explainSpec(input);
    if (result.ok) {
      throw new Error("expected a failure diagnostic but explainSpec returned ok");
    }
    return result;
  }

  it("returns ok with the normalised spec for a catalog-valid input", () => {
    const result = explainSpec(JSON.stringify(validSpec));
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.spec.root).toBe("card");
      expect(result.spec.elements?.["title"]?.type).toBe("Text");
    }
  });

  it("accepts a single ```json fenced block, mirroring resolveSpec", () => {
    const fenced = "```json\n" + JSON.stringify(mixedSpec, null, 2) + "\n```";
    const result = explainSpec(fenced);
    expect(result.ok).toBe(true);
  });

  it("strips undeclared props on success (same normalisation as resolveSpec)", () => {
    const withExtra = {
      root: "card",
      elements: {
        card: {
          type: "Card",
          props: { title: "x", bogus: "drop me" },
          children: [],
          visible: true,
        },
      },
    };
    const result = explainSpec(JSON.stringify(withExtra));
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.spec.elements?.["card"]?.props).toEqual({ title: "x" });
    }
  });

  it("reports stage 'empty' for blank or nullish input", () => {
    expect(failure("").stage).toBe("empty");
    expect(failure("   \n  ").stage).toBe("empty");
    expect(failure(null).stage).toBe("empty");
    expect(failure(undefined).stage).toBe("empty");
  });

  it("reports stage 'json' with the parser message for malformed JSON", () => {
    const result = failure('{ "root": "x", elements: }');
    expect(result.stage).toBe("json");
    expect(result.message).toContain("JSON");
  });

  it("reports stage 'shape' when the top level is not an object", () => {
    expect(failure("[]").stage).toBe("shape");
    expect(failure("42").stage).toBe("shape");
  });

  it("reports stage 'shape' when root/elements are missing", () => {
    expect(failure(JSON.stringify({ foo: 1 })).stage).toBe("shape");
  });

  it("reports stage 'components' for an unregistered component type", () => {
    const unknown = {
      root: "x",
      elements: {
        x: { type: "Carousel", props: {}, children: [], visible: true },
      },
    };
    expect(failure(JSON.stringify(unknown)).stage).toBe("components");
  });

  it("reports stage 'props' and names the offending element for bad props", () => {
    const badProps = {
      root: "x",
      elements: {
        x: {
          type: "Badge",
          props: { label: "ok", tone: "neon" },
          children: [],
          visible: true,
        },
      },
    };
    const result = failure(JSON.stringify(badProps));
    expect(result.stage).toBe("props");
    expect(result.message).toContain("Badge");
    expect(result.message).toContain('"x"');
  });

  it("reports stage 'references' for a dangling root reference", () => {
    const danglingRoot = {
      root: "missing",
      elements: {
        present: { type: "Divider", props: {}, children: [], visible: true },
      },
    };
    expect(failure(JSON.stringify(danglingRoot)).stage).toBe("references");
  });
});
