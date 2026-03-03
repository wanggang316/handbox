import { Marked } from "marked";
import { copyToClipboard, openInBrowser } from "./browser";
import { markedHighlight } from "marked-highlight";
import type { Tokens } from "marked";
import hljs from "highlight.js/lib/common";
import katex from "katex";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriEnvironment } from "./tauri";

const CODE_BLOCK_LANG_PATTERN = /\s+/;

const LANGUAGE_LABEL_MAP: Record<string, string> = {
  bash: "Bash",
  shell: "Shell",
  sh: "Shell",
  c: "C",
  cpp: "C++",
  cs: "C#",
  css: "CSS",
  docker: "Docker",
  go: "Go",
  golang: "Go",
  html: "HTML",
  java: "Java",
  javascript: "JavaScript",
  js: "JavaScript",
  json: "JSON",
  jsx: "JSX",
  kotlin: "Kotlin",
  lua: "Lua",
  markdown: "Markdown",
  md: "Markdown",
  php: "PHP",
  plaintext: "纯文本",
  python: "Python",
  py: "Python",
  ruby: "Ruby",
  rust: "Rust",
  sql: "SQL",
  swift: "Swift",
  text: "纯文本",
  toml: "TOML",
  ts: "TypeScript",
  tsx: "TSX",
  typescript: "TypeScript",
  vue: "Vue",
  xml: "XML",
  yaml: "YAML",
  yml: "YAML",
};

const COPY_BUTTON_ICON = `<svg class="code-icon" xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>`;
const COPY_BUTTON_LABEL = "已复制";

type MathTokenType = "math-inline" | "math-block";

interface MathToken extends Tokens.Generic {
  type: MathTokenType;
  raw: string;
  text: string;
  display: boolean;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function resolveLanguage(language?: string | null): string | undefined {
  if (!language) return undefined;
  const normalized = language.trim().toLowerCase();
  if (!normalized) return undefined;
  const [candidate] = normalized.split(CODE_BLOCK_LANG_PATTERN);
  return candidate || undefined;
}

function formatLanguageLabel(language?: string): string {
  if (!language) return "纯文本";
  const mapped = LANGUAGE_LABEL_MAP[language];
  if (mapped) return mapped;
  return (
    language
      .split(/[-_]/)
      .filter(Boolean)
      .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
      .join(" ") || language.toUpperCase()
  );
}

function renderMath(value: string, displayMode: boolean): string {
  try {
    return katex.renderToString(value, {
      displayMode,
      throwOnError: false,
      strict: "ignore",
      trust: true,
    });
  } catch (error) {
    if (import.meta.env.DEV) {
      console.warn("KaTeX rendering failed", { value, error });
    }
    return escapeHtml(value);
  }
}

const markedRenderer = new Marked(
  markedHighlight({
    langPrefix: "hljs language-",
    highlight(code, language) {
      const resolvedLanguage = resolveLanguage(language);

      if (resolvedLanguage && hljs.getLanguage(resolvedLanguage)) {
        try {
          return hljs.highlight(code, { language: resolvedLanguage }).value;
        } catch (error) {
          if (import.meta.env.DEV) {
            console.warn("highlight.js failed", {
              language: resolvedLanguage,
              error,
            });
          }
        }
      }

      try {
        return hljs.highlightAuto(code).value;
      } catch (error) {
        if (import.meta.env.DEV) {
          console.warn("highlight.js auto detection failed", { error });
        }
        return escapeHtml(code);
      }
    },
  }),
);

const blockMathExtension = {
  name: "math-block",
  level: "block" as const,
  start(src: string) {
    const index = src.indexOf("$$");
    return index !== -1 ? index : undefined;
  },
  tokenizer(src: string): MathToken | undefined {
    const match = src.match(/^\$\$([^]*?)\$\$\s*/);
    if (!match) return undefined;

    return {
      type: "math-block",
      raw: match[0],
      text: match[1].trim(),
      display: true,
    };
  },
  renderer(token: MathToken) {
    return `<div class="markdown-math markdown-math--block">${renderMath(token.text, true)}</div>`;
  },
};

const inlineMathExtension = {
  name: "math-inline",
  level: "inline" as const,
  start(src: string) {
    const index = src.indexOf("$");
    return index !== -1 ? index : undefined;
  },
  tokenizer(src: string): MathToken | undefined {
    const match = src.match(/^\$(?!\$)(?:(?:\\.)|[^$\n\\])+?\$/);
    if (!match) return undefined;

    const text = match[0].slice(1, -1).replace(/\\\$/g, "$").trim();

    if (!text) return undefined;

    return {
      type: "math-inline",
      raw: match[0],
      text,
      display: false,
    };
  },
  renderer(token: MathToken) {
    return `<span class="markdown-math markdown-math--inline">${renderMath(token.text, false)}</span>`;
  },
};

markedRenderer.use({
  extensions: [blockMathExtension, inlineMathExtension],
});

markedRenderer.use({
  renderer: {
    code(token: Tokens.Code) {
      const language = resolveLanguage(token.lang ?? undefined);
      const languageLabel = formatLanguageLabel(language);
      const escapedLabel = escapeHtml(languageLabel);
      const languageAttr = language
        ? ` data-language="${escapeHtml(language)}"`
        : "";
      const classNames = ["hljs"];
      if (language) {
        classNames.push(`language-${language}`);
      }

      const codeHtml = token.escaped ? token.text : escapeHtml(token.text);

      return `<figure class="markdown-code-block"${languageAttr}>
  <header class="markdown-code-block__header">
    <span class="markdown-code-block__language">${escapedLabel}</span>
    <button type="button" class="markdown-code-block__copy" aria-label="复制代码">
      <span class="markdown-code-block__copy-icon">${COPY_BUTTON_ICON}</span>
      <span class="markdown-code-block__copy-label">${COPY_BUTTON_LABEL}</span>
    </button>
  </header>
  <pre><code class="${classNames.join(" ")}">${codeHtml}\n</code></pre>
</figure>`;
    },
    image({ href, title, text }: Tokens.Image) {
      const src = resolveImageSource(href ?? "");
      const alt = escapeHtml(text ?? "");
      const titleAttr = title ? ` title="${escapeHtml(title)}"` : "";
      return `<img src="${escapeHtml(src)}" alt="${alt}"${titleAttr} />`;
    },
  },
});

markedRenderer.options({ async: false });

export function renderMarkdown(content: string | undefined | null): string {
  if (!content) return "";

  const result = markedRenderer.parse(content);
  return typeof result === "string" ? result : "";
}

const FILE_PROTOCOL = "file://";

function isLikelyAbsolutePath(value: string): boolean {
  return (
    value.startsWith("/") ||
    /^[a-zA-Z]:[\\/]/.test(value) ||
    value.startsWith("\\\\")
  );
}

function decodeFileUrlPath(raw: string): string {
  let path = raw.slice(FILE_PROTOCOL.length);
  if (/^\/[A-Za-z]:/.test(path)) {
    path = path.slice(1);
  }
  try {
    return decodeURIComponent(path);
  } catch (error) {
    console.error("Failed to decode file URL path:", { raw, path, error });
    return path;
  }
}

function resolveImageSource(rawSrc: unknown): string {
  const normalized =
    typeof rawSrc === "string" ? rawSrc : rawSrc == null ? "" : String(rawSrc);
  const trimmed = normalized.trim();

  if (!trimmed) return "";
  const lower = trimmed.toLowerCase();

  // 允许 HTTP(S) 和 data: URLs 直接通过
  if (
    lower.startsWith("http://") ||
    lower.startsWith("https://") ||
    lower.startsWith("data:")
  ) {
    return trimmed;
  }

  // 非 Tauri 环境直接返回
  if (!isTauriEnvironment()) {
    return trimmed;
  }

  try {
    // 处理 file:// URL（旧格式，兼容性处理）
    if (trimmed.startsWith(FILE_PROTOCOL)) {
      const localPath = decodeFileUrlPath(trimmed);
      if (typeof localPath !== "string" || !localPath) {
        console.error("[resolveImageSource] Invalid decoded path:", {
          trimmed,
          localPath,
        });
        return trimmed;
      }
      return convertFileSrc(localPath);
    }

    // 处理绝对路径（降级支持，不推荐）
    if (isLikelyAbsolutePath(trimmed)) {
      return convertFileSrc(trimmed);
    }
  } catch (error) {
    console.error(
      "[resolveImageSource] Failed to convert local image source:",
      {
        rawSrc,
        error,
      },
    );
  }

  return trimmed;
}

// ── Svelte Action: markdown 区域交互 ──────────────────────────────────────────

function closestButton(target: EventTarget | null): HTMLButtonElement | null {
  if (!(target instanceof Element)) return null;
  return target.closest<HTMLButtonElement>(".markdown-code-block__copy");
}

function closestLink(target: EventTarget | null): HTMLAnchorElement | null {
  if (!(target instanceof Element)) return null;
  return target.closest<HTMLAnchorElement>("a[href]");
}

function isExternalLink(link: HTMLAnchorElement): boolean {
  const href = link.getAttribute("href")?.trim();
  if (!href) return false;
  const lowerHref = href.toLowerCase();
  return (
    lowerHref.startsWith("http://") ||
    lowerHref.startsWith("https://") ||
    lowerHref.startsWith("mailto:") ||
    lowerHref.startsWith("tel:")
  );
}

async function openMarkdownLink(link: HTMLAnchorElement) {
  try {
    await openInBrowser(link.href);
  } catch (error) {
    console.error("Failed to open markdown link", error);
  }
}

/**
 * Svelte action：为 markdown 渲染区域添加交互行为。
 * - 点击代码块复制按钮：复制代码内容
 * - 点击外链：在系统浏览器中打开
 *
 * @example
 * ```svelte
 * <div use:markdownInteractions>
 *   {@html renderMarkdown(content)}
 * </div>
 * ```
 */
export function markdownInteractions(node: HTMLElement) {
  const handleClick = async (event: MouseEvent) => {
    const button = closestButton(event.target);
    if (button) {
      event.preventDefault();
      event.stopPropagation();

      const block = button.closest<HTMLElement>(".markdown-code-block");
      const codeContent = block?.querySelector("code")?.textContent ?? "";
      if (!codeContent) return;

      await copyToClipboard(codeContent);

      button.classList.add("copied");

      const timerId = button.dataset.copyTimeout
        ? Number(button.dataset.copyTimeout)
        : undefined;
      if (timerId) window.clearTimeout(timerId);

      const timeoutHandle = window.setTimeout(() => {
        button.classList.remove("copied");
        delete button.dataset.copyTimeout;
      }, 1500);

      button.dataset.copyTimeout = String(timeoutHandle);
      return;
    }

    const link = closestLink(event.target);
    if (link && isExternalLink(link)) {
      event.preventDefault();
      event.stopPropagation();
      await openMarkdownLink(link);
    }
  };

  node.addEventListener("click", handleClick);

  return {
    destroy() {
      node.removeEventListener("click", handleClick);
    },
  };
}
