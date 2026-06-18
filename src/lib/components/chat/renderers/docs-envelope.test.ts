/**
 * Cross-artifact self-consistency test (VAL-DOCS-002).
 *
 * The render-envelope protocol doc (`docs/references/render-envelope.md`) ships a
 * sample envelope under its "示例信封" heading. This test reads that sample
 * straight from the doc body, feeds it through the real `parseEnvelope` +
 * `validateTranslation`, and asserts both hit. The fixture is therefore the doc
 * itself — if someone edits a field name in the doc (or in the parser/validator)
 * and forgets the other side, this test fails. We deliberately do NOT re-author
 * an equivalent JSON literal here.
 */

// Node built-ins: available at vitest runtime (Node environment), but the
// project ships no `@types/node` and `svelte-check` runs under the SvelteKit
// tsconfig (lib: esnext/DOM only), so these specifiers have no type
// declarations there. Suppress the resolution error per import to keep
// `svelte-check` at its baseline error count without touching tsconfig.
// @ts-expect-error -- no @types/node under the svelte-check tsconfig
import { readFileSync } from "node:fs";
// @ts-expect-error -- no @types/node under the svelte-check tsconfig
import { fileURLToPath } from "node:url";
// @ts-expect-error -- no @types/node under the svelte-check tsconfig
import { dirname, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parseEnvelope } from "./envelope";
import { validateTranslation } from "./translation";

// Locate the repo root from this test file's own location (independent of the
// process CWD): renderers/ -> chat/ -> components/ -> lib/ -> src/ -> <repo>.
const thisDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(thisDir, "../../../../..");
const docPath = resolve(repoRoot, "docs/references/render-envelope.md");

/**
 * Extract the first ```json fenced block that appears after the "示例信封"
 * heading in the doc — that block is the canonical sample envelope the system
 * prompt template points at. A stable, doc-structure-anchored extraction so the
 * fixture cannot silently drift to some other code block.
 */
function extractSampleEnvelope(markdown: string): string {
  const headingIdx = markdown.indexOf("### 示例信封");
  expect(headingIdx, "doc must contain a '### 示例信封' section").toBeGreaterThanOrEqual(0);

  const afterHeading = markdown.slice(headingIdx);
  const fence = /```json\s*\n([\s\S]*?)\n```/.exec(afterHeading);
  expect(fence, "the 示例信封 section must contain a ```json fenced block").not.toBeNull();

  return (fence as RegExpExecArray)[1].trim();
}

describe("render-envelope protocol doc (self-consistency)", () => {
  const markdown = readFileSync(docPath, "utf8");

  it("documents the discriminator, translation schema fields, and registry convention (VAL-DOCS-001)", () => {
    expect(markdown).toContain("__render");
    // translation data schema field names must be documented verbatim.
    for (const field of ["term", "translation", "phonetic", "explanation"]) {
      expect(markdown, `doc must document the '${field}' field`).toContain(field);
    }
    // Registry-extension convention must be described.
    expect(markdown).toContain("src/lib/components/chat/renderers/");
    expect(markdown).toContain("rendererRegistry.register");
  });

  it("parses the doc's sample envelope and passes translation validation (VAL-DOCS-002)", () => {
    const sample = extractSampleEnvelope(markdown);

    const envelope = parseEnvelope(sample);
    expect(envelope).not.toBeNull();
    expect(envelope?.type).toBe("translation");

    const data = validateTranslation(envelope?.data);
    expect(data).not.toBeNull();
    // The doc's required field must survive validation as a non-empty string.
    expect(typeof data?.translation).toBe("string");
    expect((data?.translation ?? "").trim().length).toBeGreaterThan(0);
  });
});
