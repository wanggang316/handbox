/**
 * Guards the curated GenUI starter templates: every example must round-trip
 * through the same `explainSpec` pipeline the editor and chat use, so a typo in
 * a spec (bad enum, missing required prop, dangling child id) fails CI rather
 * than silently shipping a template that won't render.
 *
 * Pure TypeScript — `examples.ts` imports only a type from `@json-render/core`,
 * and `resolveSpec.ts` is `.svelte`-free, so this runs under plain-Node Vitest.
 */

import { describe, it, expect } from "vitest";
import { explainSpec } from "../chat/renderers/jsonui/resolveSpec";
import { genuiExamples } from "./examples";

describe("genuiExamples", () => {
  it("is non-empty", () => {
    expect(genuiExamples.length).toBeGreaterThan(0);
  });

  it("every example resolves to a renderable spec", () => {
    for (const ex of genuiExamples) {
      const result = explainSpec(JSON.stringify(ex.spec));
      const reason = result.ok ? "" : `${result.stage}: ${result.message}`;
      expect(result.ok, `example "${ex.id}" must be valid — ${reason}`).toBe(
        true,
      );
    }
  });

  it("has unique, non-empty ids/names and a description each", () => {
    const ids = genuiExamples.map((e) => e.id);
    const names = genuiExamples.map((e) => e.name);
    expect(new Set(ids).size, "ids must be unique").toBe(ids.length);
    expect(new Set(names).size, "names must be unique").toBe(names.length);
    for (const ex of genuiExamples) {
      expect(ex.id.trim().length).toBeGreaterThan(0);
      expect(ex.name.trim().length).toBeGreaterThan(0);
      expect(ex.description.trim().length).toBeGreaterThan(0);
    }
  });
});
