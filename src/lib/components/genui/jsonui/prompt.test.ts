/**
 * Unit tests for the frozen generative-UI system prompt.
 *
 * Pure TypeScript — imports `prompt.ts` / `catalog.ts` / `resolveSpec.ts`, none
 * of which pulls in a `.svelte` module, so the suite runs under the plain-Node
 * Vitest environment.
 *
 * Three contracts are pinned here:
 *
 *  - **VAL-CATALOG-008** — the prompt names every catalog component (the six
 *    expansion components included) AND carries each one's catalog description.
 *  - **VAL-CATALOG-009** — the prompt instructs *whole-spec* output (root +
 *    elements), carries none of the patch/JSONL wording our resolver rejects,
 *    and its embedded example round-trips through `resolveSpec` to a non-null
 *    spec.
 *  - **VAL-INJECT-005 (drift)** — the committed
 *    `src-tauri/resources/generative-ui-prompt.txt` is byte-identical to the
 *    builder's output, so a catalog change without regenerating the file fails CI.
 */

// @ts-expect-error -- no @types/node under the svelte-check tsconfig
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";
import { buildGenerativeUiPrompt } from "./prompt";
import { uiCatalog } from "./catalog";
import { resolveSpec } from "./resolveSpec";

/** The six presentational components added by the catalog-expansion milestone. */
const EXPANSION_COMPONENTS = [
  "StatusLabel",
  "Avatar",
  "Divider",
  "KeyValue",
  "Table",
  "InfoTooltip",
] as const;

describe("buildGenerativeUiPrompt — component coverage (VAL-CATALOG-008)", () => {
  const prompt = buildGenerativeUiPrompt(uiCatalog);
  const descriptions = uiCatalog.data.components as Record<
    string,
    { description?: string }
  >;

  it("names every catalog component, including the six expansion components", () => {
    for (const name of uiCatalog.componentNames) {
      expect(prompt).toContain(name);
    }
    for (const name of EXPANSION_COMPONENTS) {
      expect(prompt).toContain(name);
    }
  });

  it("carries each expansion component's description text verbatim", () => {
    for (const name of EXPANSION_COMPONENTS) {
      const description = descriptions[name]?.description ?? "";
      expect(description.length).toBeGreaterThan(0);
      expect(prompt).toContain(description);
    }
  });
});

describe("buildGenerativeUiPrompt — whole-spec contract (VAL-CATALOG-009)", () => {
  const prompt = buildGenerativeUiPrompt(uiCatalog);

  it("instructs whole-spec output (root + elements)", () => {
    expect(prompt).toContain('"root"');
    expect(prompt).toContain('"elements"');
  });

  it("contains no patch / JSONL / streaming-patch wording", () => {
    const lowered = prompt.toLowerCase();
    expect(lowered).not.toContain("jsonl");
    expect(lowered).not.toContain("rfc 6902");
    expect(lowered).not.toContain("rfc6902");
    expect(lowered).not.toContain("patch");
    expect(prompt).not.toContain('"op"');
  });

  it("embeds an example that resolveSpec accepts as a non-null spec", () => {
    const example = extractEmbeddedExample(prompt);
    expect(example).not.toBeNull();
    const resolved = resolveSpec(example as string);
    expect(resolved).not.toBeNull();
    expect(resolved?.root).toBeTypeOf("string");
  });
});

describe("generative-ui-prompt.txt drift guard (VAL-INJECT-005)", () => {
  it("the committed resource is byte-identical to the builder output", () => {
    // Path is relative to the repo root = the Vitest working directory.
    const committed = readFileSync(
      "src-tauri/resources/generative-ui-prompt.txt",
      "utf8",
    );
    expect(committed).toBe(buildGenerativeUiPrompt(uiCatalog));
  });
});

/**
 * Recover the embedded example spec from the prompt: the single ```json fenced
 * block. Returned as the fenced carrier string (which `resolveSpec` accepts
 * directly), or `null` when no fenced block is present.
 */
function extractEmbeddedExample(prompt: string): string | null {
  const match = /```json\n[\s\S]*?\n```/.exec(prompt);
  return match === null ? null : match[0];
}
