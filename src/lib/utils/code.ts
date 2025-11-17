import hljs from "highlight.js/lib/common";

interface RenderCodeOptions {
  language?: string;
  variant?: "default" | "compact";
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function highlightContent(content: string, language?: string): string {
  if (!content) return "";

  try {
    if (language && hljs.getLanguage(language)) {
      return hljs.highlight(content, { language }).value;
    }

    const auto = hljs.highlightAuto(content);
    return auto.value;
  } catch (error) {
    return escapeHtml(content);
  }
}

export function renderCodeBlock(
  content: string,
  { language = "", variant = "default" }: RenderCodeOptions = {}
): string {
  const preClasses = ["hljs", "code-block"];
  if (variant === "compact") {
    preClasses.push("code-block--compact");
  }

  const codeClasses = ["hljs"];
  if (language) {
    codeClasses.push(`language-${language}`);
  }

  const highlighted = highlightContent(content, language || undefined);

  return `<pre class="${preClasses.join(" ")}"><code class="${codeClasses.join(
    " "
  )}">${highlighted}</code></pre>`;
}

