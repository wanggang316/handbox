<script lang="ts">
  // Sandboxed HTML host: renders model-produced HTML inside a srcdoc iframe.
  //
  // Isolation contract (mirrors hand-ai's sandboxed-iframe):
  //  - sandbox="allow-scripts allow-modals" — deliberately NO allow-same-origin
  //    (opaque origin: the content can never script the parent, read cookies,
  //    or reach Tauri IPC) and NO allow-top-navigation.
  //  - The app CSP (script-src 'self' 'unsafe-inline'; connect-src ipc:) is
  //    inherited by the srcdoc document, so external scripts/styles/fetches are
  //    blocked structurally — cards must be self-contained.
  //
  // Theming: a snapshot of the app's short-alias CSS variables is injected into
  // the document so cards can consume var(--base-100) etc. The snapshot is
  // rebuilt when data-theme changes (iframe reload on theme switch is rare and
  // acceptable).
  //
  // Height: with an opaque origin the parent cannot read the content's height,
  // so an injected ResizeObserver posts `handbox:resize` messages upward;
  // the listener validates `event.source` against this iframe's contentWindow.

  interface Props {
    /** Self-contained HTML: a fragment or a complete document. */
    html: string;
    /**
     * inline — height always follows content (no clamp: an inner scrollbar
     * would break the "card blends into the conversation" contract; the page
     * scrolls instead), for in-timeline cards; fill — stretches to the
     * container (artifact panel, M2).
     */
    mode?: "inline" | "fill";
    /** Accessible iframe title. */
    title?: string;
  }

  let { html, mode = "inline", title = "HTML card" }: Props = $props();

  let iframeEl: HTMLIFrameElement | undefined = $state();
  let height = $state(120);
  // Bumped when data-theme flips so the srcdoc (and its theme snapshot) rebuilds.
  let themeVersion = $state(0);

  /** App short-alias variables mirrored into the sandbox (see app.css :root). */
  const THEME_VARS = [
    "--base-100",
    "--base-200",
    "--base-300",
    "--base-content",
    "--primary",
    "--primary-content",
    "--secondary",
    "--secondary-content",
    "--accent",
    "--accent-content",
    "--neutral",
    "--neutral-content",
    "--info",
    "--success",
    "--warning",
    "--error",
    "--overlay",
    "--hairline",
  ];

  function themeSnapshot(): string {
    const style = getComputedStyle(document.documentElement);
    const decls = THEME_VARS.map((name) => {
      const value = style.getPropertyValue(name).trim();
      return value ? `${name}:${value};` : "";
    }).join("");
    return `:root{${decls}}`;
  }

  // Injected reporter: content height → parent. Load + ResizeObserver covers
  // both static cards and ones that grow after interaction.
  const RESIZE_RUNTIME = `
<script>(function(){
  function report(){
    var d = document.documentElement;
    var h = Math.max(d.scrollHeight, document.body ? document.body.scrollHeight : 0);
    parent.postMessage({ type: "handbox:resize", height: h }, "*");
  }
  var ro = new ResizeObserver(report);
  ro.observe(document.documentElement);
  if (document.body) ro.observe(document.body);
  window.addEventListener("load", report);
  report();
})();<\/script>`;

  function buildSrcdoc(userHtml: string): string {
    const themeStyle = `<style>${themeSnapshot()}</style>`;
    const runtime = mode === "inline" ? RESIZE_RUNTIME : "";

    // Complete document: inject the theme + runtime right after <head> (or
    // <html> when headless) so the card's own styles can still override.
    if (/<html[\s>]/i.test(userHtml)) {
      const injection = themeStyle + runtime;
      const headMatch = userHtml.match(/<head[^>]*>/i);
      if (headMatch && headMatch.index !== undefined) {
        const at = headMatch.index + headMatch[0].length;
        return userHtml.slice(0, at) + injection + userHtml.slice(at);
      }
      const htmlMatch = userHtml.match(/<html[^>]*>/i);
      if (htmlMatch && htmlMatch.index !== undefined) {
        const at = htmlMatch.index + htmlMatch[0].length;
        return userHtml.slice(0, at) + injection + userHtml.slice(at);
      }
      return injection + userHtml;
    }

    // Fragment: wrap in a minimal skeleton. Transparent body + zero padding:
    // the card has no chrome, so the generated HTML owns the full width and
    // all of its own spacing.
    return (
      "<!DOCTYPE html><html><head><meta charset=\"utf-8\">" +
      themeStyle +
      "<style>body{margin:0;padding:0;background:transparent;color:var(--base-content);" +
      'font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;font-size:14px;line-height:1.6;}</style>' +
      "</head><body>" +
      userHtml +
      runtime +
      "</body></html>"
    );
  }

  const srcdoc = $derived.by(() => {
    void themeVersion;
    return buildSrcdoc(html);
  });

  // Rebuild the theme snapshot when the app theme flips.
  $effect(() => {
    const observer = new MutationObserver(() => {
      themeVersion += 1;
    });
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  });

  // Height messages: only trust this iframe's own contentWindow.
  $effect(() => {
    if (mode !== "inline") return;
    const onMessage = (event: MessageEvent) => {
      if (!iframeEl || event.source !== iframeEl.contentWindow) return;
      const data: unknown = event.data;
      if (
        typeof data === "object" &&
        data !== null &&
        (data as { type?: unknown }).type === "handbox:resize" &&
        typeof (data as { height?: unknown }).height === "number"
      ) {
        const reported = Math.ceil((data as { height: number }).height);
        height = Math.max(reported, 40);
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  });
</script>

{#if mode === "inline"}
  <iframe
    bind:this={iframeEl}
    {srcdoc}
    {title}
    sandbox="allow-scripts allow-modals"
    class="block w-full border-0"
    style:height={`${height}px`}
  ></iframe>
{:else}
  <iframe
    bind:this={iframeEl}
    {srcdoc}
    {title}
    sandbox="allow-scripts allow-modals"
    class="block h-full w-full border-0"
  ></iframe>
{/if}
