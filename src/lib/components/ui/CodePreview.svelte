<script lang="ts">
  /**
   * Read-only source view: a line-number gutter beside the code, which scrolls
   * sideways rather than wrapping. Wrapping a long line destroys the alignment
   * that makes code scannable, and it makes the line numbers lie.
   */
  import hljs from "highlight.js/lib/common";

  interface Props {
    code: string;
    /** File name or path; its extension picks the grammar. */
    filename?: string;
  }

  let { code, filename = "" }: Props = $props();

  /** Extensions whose name differs from the highlight.js grammar. */
  const LANGUAGE_BY_EXT: Record<string, string> = {
    js: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    jsx: "javascript",
    ts: "typescript",
    tsx: "typescript",
    py: "python",
    rb: "ruby",
    rs: "rust",
    kt: "kotlin",
    h: "c",
    hpp: "cpp",
    cs: "csharp",
    sh: "bash",
    zsh: "bash",
    yml: "yaml",
    mdx: "markdown",
    env: "bash",
  };

  function escapeHtml(value: string): string {
    return value
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;");
  }

  const lineNumbers = $derived(
    Array.from({ length: code.split("\n").length }, (_, i) => String(i + 1)).join(
      "\n",
    ),
  );

  // Only grammars named by the extension are highlighted. Auto-detection is a
  // guess that costs a full pass over the file, which on the largest file this
  // view will open is a visible stall for a result that is often wrong.
  const highlighted = $derived.by(() => {
    const ext = filename.split(".").pop()?.toLowerCase() ?? "";
    const language = LANGUAGE_BY_EXT[ext] ?? ext;
    if (!language || !hljs.getLanguage(language)) return escapeHtml(code);
    try {
      return hljs.highlight(code, { language }).value;
    } catch (error) {
      console.warn("highlight.js failed", { language, error });
      return escapeHtml(code);
    }
  });
</script>

<div class="code-preview">
  <pre class="code-preview__gutter" aria-hidden="true">{lineNumbers}</pre>
  <div class="code-preview__scroller">
    <pre class="code-preview__code"><code class="hljs">{@html highlighted}</code
      ></pre>
  </div>
</div>

<style>
  .code-preview {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
  }

  .code-preview__gutter,
  .code-preview__code {
    margin: 0;
    font-family: ui-monospace, "SF Mono", Menlo, Monaco, Consolas, monospace;
    font-size: 0.75rem;
    line-height: 1.6;
  }

  .code-preview__gutter {
    padding: 0 0.75rem 0 0;
    text-align: right;
    color: color-mix(in oklch, var(--base-content) 35%, transparent);
    white-space: pre;
    user-select: none;
    border-right: 1px solid var(--hairline);
  }

  .code-preview__scroller {
    min-width: 0;
    overflow-x: auto;
  }

  .code-preview__code {
    /* max-content + min-width keeps short files flush left while long lines
       still push the scroller wider than the pane. */
    width: max-content;
    min-width: 100%;
    padding: 0 0 0 0.75rem;
    white-space: pre;
    color: var(--base-content);
  }

  .code-preview__code code.hljs {
    display: block;
    font: inherit;
    line-height: inherit;
    white-space: inherit;
    padding: 0;
    background: transparent;
    color: inherit;
  }
</style>
