/**
 * Frozen "generative-UI" system-prompt builder.
 *
 * {@link buildGenerativeUiPrompt} turns the {@link uiCatalog} into a single,
 * deterministic instruction block that teaches an LLM to emit a *whole* spec —
 * one complete JSON object `{ root, elements }` — using only the catalog's
 * presentational components.
 *
 * Two deliberate departures from json-render's stock `uiCatalog.prompt()`:
 *
 *  1. **Whole-spec, not streaming-patch.** The stock prompt is JSONL / RFC 6902
 *     JSON-Patch oriented (`{"op":"add",...}` per line). Our {@link resolveSpec}
 *     accepts only one complete spec object, so this prompt instructs the model
 *     to output exactly that and never mentions `op`/JSONL/patch wording.
 *  2. **Self-contained.** Only the per-component signature lines (type name,
 *     prop shape, description) are reused from `uiCatalog.prompt()`; all framing,
 *     the worked example, and the output contract are authored here.
 *
 * The output is DETERMINISTIC: components are emitted in `uiCatalog.componentNames`
 * order (a fixed array), and the embedded example is a constant. This stability
 * is what lets the committed `generative-ui-prompt.txt` be drift-checked
 * byte-for-byte against this builder's output.
 *
 * Pure TypeScript — imports only `catalog.ts` (no `.svelte`), so it is safe to
 * pull into the Node-environment unit tests and the Node generator script.
 */

import { uiCatalog } from "./catalog";

/**
 * The worked example handed to the model, also embedded verbatim in the prompt.
 *
 * It is a clean, catalog-valid whole spec (Card → Stack → Text/StatusLabel/Badge)
 * that {@link resolveSpec} accepts. The drift/content test extracts this exact
 * object from the prompt and asserts `resolveSpec(example)` is non-null, so the
 * shape here must stay valid: every element carries `children` and `visible`,
 * and every component's required props are present.
 */
const EXAMPLE_SPEC = {
  root: "card",
  elements: {
    card: {
      type: "Card",
      props: { title: "Service status" },
      children: ["stack"],
      visible: true,
    },
    stack: {
      type: "Stack",
      props: { gap: "md", direction: "col" },
      children: ["heading", "status", "tag"],
      visible: true,
    },
    heading: {
      type: "Text",
      props: { text: "Realtime sync", variant: "heading" },
      children: [],
      visible: true,
    },
    status: {
      type: "StatusLabel",
      props: { status: "enabled", text: "Online" },
      children: [],
      visible: true,
    },
    tag: {
      type: "Badge",
      props: { label: "v2", tone: "info" },
      children: [],
      visible: true,
    },
  },
};

/**
 * Extract the per-component signature lines from `uiCatalog.prompt()`. Each line
 * has the json-render-maintained shape
 * `- Name: { prop?: type, ... } - description [accepts children]`, carrying the
 * component's type name, prop names/types/enums, and its catalog description.
 *
 * Returned keyed by type name so the caller can re-emit them in a fixed order
 * (the parse order of `prompt()` is not contractually stable; the caller's
 * `componentNames` order is).
 */
function componentSignatureLines(): Map<string, string> {
  const byType = new Map<string, string>();
  for (const raw of uiCatalog.prompt().split("\n")) {
    const line = raw.trimEnd();
    const match = /^- ([A-Za-z][A-Za-z0-9]*): \{/.exec(line);
    if (match !== null) {
      byType.set(match[1], line);
    }
  }
  return byType;
}

/**
 * Build the frozen generative-UI system prompt from the catalog.
 *
 * The output literally contains, for every catalog component, its type name and
 * its `description` text; instructs whole-spec JSON output (`root` + `elements`)
 * with no patch/JSONL wording; and embeds {@link EXAMPLE_SPEC} as a fenced
 * ```json block that {@link resolveSpec} accepts.
 *
 * @param catalog The UI catalog (defaults to the shared {@link uiCatalog}); the
 *   parameter exists so the drift test can pass the same instance explicitly.
 */
export function buildGenerativeUiPrompt(
  catalog: typeof uiCatalog = uiCatalog,
): string {
  const signatures = componentSignatureLines();
  const components = catalog.data.components as Record<
    string,
    { description?: string }
  >;

  const componentBlock = catalog.componentNames
    .map((name) => {
      const signature = signatures.get(name);
      if (signature !== undefined) {
        return signature;
      }
      // Fallback (should not happen for catalog components): synthesise a line
      // from the description alone so the type name + description still appear.
      const description = components[name]?.description ?? "";
      return `- ${name}: {} - ${description}`;
    })
    .join("\n");

  const exampleBlock = "```json\n" + JSON.stringify(EXAMPLE_SPEC, null, 2) + "\n```";

  return [
    "You are a UI generator for HandBox. When a reply is best shown as a small",
    "structured card rather than prose, respond with a generative-UI spec.",
    "",
    "OUTPUT FORMAT:",
    "Output a single, complete JSON object describing the whole UI in one shot.",
    "The object has exactly two top-level fields:",
    '  - "root": the id of the top-level element (a string).',
    '  - "elements": an object mapping each element id to its definition.',
    "Output the entire spec at once as one JSON value. Do not split it across",
    "multiple lines as separate JSON values, and do not emit incremental edits.",
    "",
    "Each element in \"elements\" is an object with these fields:",
    '  - "type": the component name (must be one of the AVAILABLE COMPONENTS below).',
    '  - "props": an object of that component\'s props.',
    '  - "children": an array of child element ids (use [] for leaf components).',
    '  - "visible": a boolean; use true unless the element should be hidden.',
    'Every element MUST include "children" (an array) and "visible" (a boolean).',
    "The root id and every id listed in any \"children\" array MUST exist as a key",
    'in "elements".',
    "",
    "Wrap the JSON object in a single ```json fenced code block and output nothing",
    "else — no surrounding prose, no explanation, no extra text.",
    "",
    `AVAILABLE COMPONENTS (${catalog.componentNames.length}):`,
    "",
    componentBlock,
    "",
    "EXAMPLE:",
    "A Card containing a Stack with a heading, a status label, and a badge.",
    "",
    exampleBlock,
    "",
    "Only use the component types listed above. Keep specs small and focused.",
    "If a structured card does not fit the request, answer in plain text instead.",
    "",
  ].join("\n");
}
