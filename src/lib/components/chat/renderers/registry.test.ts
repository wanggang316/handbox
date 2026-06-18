import { describe, expect, it } from "vitest";
import { RendererRegistry } from "./registry";
import { validateTranslation } from "./translation";
import type { Renderer, TranslationData } from "./types";

// Lightweight placeholder components. The registry never inspects `component`,
// so a tagged sentinel object is enough to assert dispatch without pulling in a
// `.svelte` file (which the Node test environment cannot compile).
const TRANSLATION_COMPONENT = { name: "TranslationCard" };
const FAKE_COMPONENT = { name: "FakeCard" };
const OVERRIDE_COMPONENT = { name: "OverrideCard" };

/** A translation renderer wired to the real validator and a placeholder component. */
function makeTranslationRenderer(): Renderer<TranslationData, typeof TRANSLATION_COMPONENT> {
  return {
    type: "translation",
    validate: validateTranslation,
    component: TRANSLATION_COMPONENT,
  };
}

describe("RendererRegistry", () => {
  // ---------------------------------------------------------------------------
  // Group 1 — registered type hits and returns validated data (VAL-REGISTRY-001)
  // ---------------------------------------------------------------------------
  describe("registered-type dispatch", () => {
    it("resolves 'translation' to the registered renderer", () => {
      const registry = new RendererRegistry();
      const renderer = makeTranslationRenderer();
      registry.register(renderer);

      expect(registry.lookup("translation")).toBe(renderer);
    });

    it("the resolved renderer validates its payload, preserving extra fields tolerance", () => {
      const registry = new RendererRegistry();
      registry.register(makeTranslationRenderer());

      const resolved = registry.lookup("translation");
      expect(resolved).not.toBeNull();

      // Payload carries an unknown extra field — it is ignored, not rejected.
      const data = resolved?.validate({ translation: "你好", note: "extra" });
      expect(data).toEqual({ translation: "你好" });
    });
  });

  // ---------------------------------------------------------------------------
  // Group 2 — non-hits never throw, return null (VAL-REGISTRY-002)
  // ---------------------------------------------------------------------------
  describe("non-hits return null without throwing", () => {
    it("returns null for an unknown type", () => {
      const registry = new RendererRegistry();
      registry.register(makeTranslationRenderer());

      expect(registry.lookup("unknown")).toBeNull();
    });

    it("is case-sensitive: 'Translation' and 'TRANSLATION' do not match 'translation'", () => {
      const registry = new RendererRegistry();
      registry.register(makeTranslationRenderer());

      expect(registry.lookup("Translation")).toBeNull();
      expect(registry.lookup("TRANSLATION")).toBeNull();
    });

    it("returns null from an empty registry", () => {
      const registry = new RendererRegistry();

      expect(registry.lookup("translation")).toBeNull();
    });

    it("returns null for undefined / null / empty-string type without throwing", () => {
      const registry = new RendererRegistry();
      registry.register(makeTranslationRenderer());

      expect(() => registry.lookup(undefined)).not.toThrow();
      expect(() => registry.lookup(null)).not.toThrow();
      expect(() => registry.lookup("")).not.toThrow();

      expect(registry.lookup(undefined)).toBeNull();
      expect(registry.lookup(null)).toBeNull();
      expect(registry.lookup("")).toBeNull();
    });
  });

  // ---------------------------------------------------------------------------
  // Group 3 — extensibility: a second renderer dispatches; original unaffected
  //           (VAL-REGISTRY-004)
  // ---------------------------------------------------------------------------
  describe("extensibility via a second renderer", () => {
    it("dispatches the new type to its component while leaving translation intact", () => {
      const registry = new RendererRegistry();
      const translation = makeTranslationRenderer();
      const fake: Renderer<{ ok: boolean }, typeof FAKE_COMPONENT> = {
        type: "fake",
        validate: (data) =>
          typeof data === "object" && data !== null ? { ok: true } : null,
        component: FAKE_COMPONENT,
      };
      registry.register(translation);
      registry.register(fake);

      expect(registry.lookup("fake")?.component).toBe(FAKE_COMPONENT);
      // The original renderer is undisturbed by the second registration.
      expect(registry.lookup("translation")).toBe(translation);
      expect(registry.lookup("translation")?.component).toBe(TRANSLATION_COMPONENT);
    });
  });

  // ---------------------------------------------------------------------------
  // Group 4 — re-registering a type: last writer wins (VAL-REGISTRY-005)
  // ---------------------------------------------------------------------------
  describe("re-registering the same type", () => {
    it("the later registration overwrites the earlier one", () => {
      const registry = new RendererRegistry();
      const first = makeTranslationRenderer();
      const second: Renderer<TranslationData, typeof OVERRIDE_COMPONENT> = {
        type: "translation",
        validate: validateTranslation,
        component: OVERRIDE_COMPONENT,
      };
      registry.register(first);
      registry.register(second);

      const resolved = registry.lookup("translation");
      expect(resolved).toBe(second);
      expect(resolved?.component).toBe(OVERRIDE_COMPONENT);
    });
  });
});

// =============================================================================
// validateTranslation — payload shape gate (VAL-REGISTRY-003)
// =============================================================================
describe("validateTranslation", () => {
  describe("translation field (required)", () => {
    it("accepts a non-empty translation string", () => {
      expect(validateTranslation({ translation: "hello" })).toEqual({
        translation: "hello",
      });
    });

    it("rejects a missing translation", () => {
      expect(validateTranslation({ term: "word" })).toBeNull();
    });

    it("rejects an empty-string translation", () => {
      expect(validateTranslation({ translation: "" })).toBeNull();
    });

    it("rejects a whitespace-only translation", () => {
      expect(validateTranslation({ translation: "   \n\t " })).toBeNull();
    });

    it("rejects a non-string translation", () => {
      expect(validateTranslation({ translation: 42 })).toBeNull();
      expect(validateTranslation({ translation: null })).toBeNull();
      expect(validateTranslation({ translation: { x: 1 } })).toBeNull();
    });

    it("preserves leading/trailing whitespace in a valid translation verbatim", () => {
      // Non-empty-after-trim is the gate; the value itself is not trimmed.
      expect(validateTranslation({ translation: "  hi  " })).toEqual({
        translation: "  hi  ",
      });
    });
  });

  describe("non-object input", () => {
    it("rejects undefined (the parser's no-data envelope)", () => {
      expect(validateTranslation(undefined)).toBeNull();
    });

    it("rejects null", () => {
      expect(validateTranslation(null)).toBeNull();
    });

    it("rejects an array", () => {
      expect(validateTranslation([{ translation: "x" }])).toBeNull();
    });

    it("rejects primitives", () => {
      expect(validateTranslation("translation")).toBeNull();
      expect(validateTranslation(42)).toBeNull();
      expect(validateTranslation(true)).toBeNull();
    });
  });

  describe("optional fields", () => {
    it("carries through term / phonetic / explanation when they are strings", () => {
      expect(
        validateTranslation({
          translation: "你好",
          term: "hello",
          phonetic: "ˈheˈləʊ",
          explanation: "a greeting",
        }),
      ).toEqual({
        translation: "你好",
        term: "hello",
        phonetic: "ˈheˈləʊ",
        explanation: "a greeting",
      });
    });

    it("passes when optional fields are absent", () => {
      const result = validateTranslation({ translation: "x" });
      expect(result).toEqual({ translation: "x" });
      expect(result).not.toHaveProperty("term");
      expect(result).not.toHaveProperty("phonetic");
      expect(result).not.toHaveProperty("explanation");
    });

    it("drops non-string optional fields (key omitted, payload still valid)", () => {
      const result = validateTranslation({
        translation: "x",
        term: 123,
        phonetic: null,
        explanation: { md: true },
      });
      expect(result).toEqual({ translation: "x" });
      expect(result).not.toHaveProperty("term");
      expect(result).not.toHaveProperty("phonetic");
      expect(result).not.toHaveProperty("explanation");
    });

    it("keeps the string optional fields while dropping the non-string ones", () => {
      const result = validateTranslation({
        translation: "x",
        term: "kept",
        phonetic: 99,
      });
      expect(result).toEqual({ translation: "x", term: "kept" });
      expect(result).not.toHaveProperty("phonetic");
    });
  });

  describe("extra fields", () => {
    it("ignores unknown fields without failing", () => {
      expect(
        validateTranslation({ translation: "x", foo: "bar", count: 7 }),
      ).toEqual({ translation: "x" });
    });
  });

  it("never throws across assorted malformed inputs", () => {
    const inputs: unknown[] = [
      undefined,
      null,
      0,
      "",
      [],
      {},
      { translation: [] },
      { translation: {} },
      Symbol("x"),
    ];
    for (const input of inputs) {
      expect(() => validateTranslation(input)).not.toThrow();
    }
  });
});
