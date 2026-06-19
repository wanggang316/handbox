/**
 * Generator for the frozen generative-UI system prompt.
 *
 * Writes `buildGenerativeUiPrompt(uiCatalog)`'s output to
 * `src-tauri/resources/generative-ui-prompt.txt`, the file the Rust backend
 * `include_str!`s and injects into LLM requests for `generative_ui` sessions.
 *
 * Run via the `gen:gui-prompt` npm script (`vite-node`). The output is
 * deterministic, so re-running after a catalog change is how you refresh the
 * committed `.txt`; the vitest drift test fails CI if the committed file and the
 * builder output diverge.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { buildGenerativeUiPrompt } from "../src/lib/components/chat/renderers/jsonui/prompt";
import { uiCatalog } from "../src/lib/components/chat/renderers/jsonui/catalog";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const outPath = resolve(repoRoot, "src-tauri/resources/generative-ui-prompt.txt");

const prompt = buildGenerativeUiPrompt(uiCatalog);

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, prompt, "utf8");

console.log(`Wrote ${prompt.length} bytes to ${outPath}`);
