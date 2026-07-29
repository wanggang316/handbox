/**
 * Frozen "generative-UI" system-prompt builder: turns {@link uiCatalog} into the
 * instruction block that teaches an LLM to emit one complete `{ root, elements }`
 * spec.
 *
 * It deliberately departs from json-render's stock `uiCatalog.prompt()`, which is
 * JSONL / JSON-Patch oriented: {@link resolveSpec} accepts only a whole spec
 * object, so only the per-component signature lines are reused and all framing,
 * the worked example, and the output contract are authored here.
 *
 * The output is DETERMINISTIC — fixed `componentNames` order, constant example —
 * which is what lets the committed `generative-ui-prompt.txt` be drift-checked
 * byte-for-byte against this builder.
 *
 * Imports no `.svelte`, so Node tests and the Node generator script can use it.
 */

import { uiCatalog } from "./catalog";

/**
 * The worked example embedded verbatim in the prompt. The drift test extracts it
 * back out and asserts {@link resolveSpec} accepts it, so it must stay
 * catalog-valid: every element carries `children` and `visible`, and every
 * component's required props are present.
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
 * Extract the per-component signature lines from `uiCatalog.prompt()`, each of
 * the json-render-maintained shape
 * `- Name: { prop?: type, ... } - description [accepts children]`. Keyed by type
 * name because `prompt()`'s parse order is not contractually stable, whereas the
 * caller's `componentNames` order is.
 */
function componentSignatureLines(
  catalog: typeof uiCatalog,
): Map<string, string> {
  const byType = new Map<string, string>();
  for (const raw of catalog.prompt().split("\n")) {
    const line = raw.trimEnd();
    const match = /^- ([A-Za-z][A-Za-z0-9]*): \{/.exec(line);
    if (match !== null) {
      byType.set(match[1], line);
    }
  }
  return byType;
}

/**
 * Build the frozen generative-UI system prompt from the catalog. `catalog` is
 * injectable only so the drift test can pass the instance explicitly.
 */
export function buildGenerativeUiPrompt(
  catalog: typeof uiCatalog = uiCatalog,
): string {
  const signatures = componentSignatureLines(catalog);
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
      // Unreachable for catalog components; keep type name + description anyway.
      const description = components[name]?.description ?? "";
      return `- ${name}: {} - ${description}`;
    })
    .join("\n");

  const exampleBlock =
    "```json\n" + JSON.stringify(EXAMPLE_SPEC, null, 2) + "\n```";

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
    'Each element in "elements" is an object with these fields:',
    '  - "type": the component name (must be one of the AVAILABLE COMPONENTS below).',
    '  - "props": an object of that component\'s props.',
    '  - "children": an array of child element ids (use [] for leaf components).',
    '  - "visible": a boolean; use true unless the element should be hidden.',
    'Every element MUST include "children" (an array) and "visible" (a boolean).',
    'The root id and every id listed in any "children" array MUST exist as a key',
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
