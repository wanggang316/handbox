<script lang="ts">
  // Timeline pill for `render_app` toolcall blocks (feat/html-segment-render).
  //
  // The app itself lives in the right-side panel (AppPanel); the timeline only
  // shows a compact, clickable pill — mirroring hand-ai's artifact pill. Args
  // become available at tool_execution_start, so the pill shows the real title
  // while the call is still `executing`; errors get a compact error box like
  // HtmlCard.
  import { AppWindow, XCircle } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import type { ToolCallView } from "$lib/states/agentRun.svelte";
  import { agentAppPanel } from "$lib/states/agentAppPanel.svelte";
  import { parseRenderAppArgs } from "./renderApp";

  interface Props {
    toolCall: ToolCallView;
    sessionId: string;
    /** Folded artifact title — labels title-less `update` pills with the app's
     * current name instead of "untitled" (every pill opens the same panel). */
    fallbackTitle?: string;
  }

  let { toolCall, sessionId, fallbackTitle }: Props = $props();

  const parsed = $derived(parseRenderAppArgs(toolCall.args));
  const title = $derived(
    parsed?.title || fallbackTitle || t("agent.htmlApp.untitled"),
  );
</script>

{#if toolCall.status === "error"}
  <div
    class="flex items-center gap-2 rounded-lg border border-error/40 bg-error/10 px-3 py-2 text-sm text-error"
  >
    <XCircle size={14} class="shrink-0" />
    <span>{t("agent.htmlApp.error")}</span>
  </div>
{:else if parsed}
  <button
    type="button"
    class="flex w-full items-center gap-2 rounded-lg border border-[var(--hairline)] bg-base-100 px-3 py-2 text-left text-sm transition-colors hover:bg-base-200"
    onclick={() => agentAppPanel.open(sessionId)}
  >
    <AppWindow size={16} class="shrink-0 text-base-content/60" />
    <span class="min-w-0 flex-1 truncate text-base-content">{title}</span>
    {#if toolCall.status === "executing"}
      <span
        class="h-3 w-3 shrink-0 rounded-full bg-base-content/50 animate-[pulse-scale_1.5s_ease-in-out_infinite]"
      ></span>
    {:else}
      <span class="shrink-0 text-xs text-base-content/40"
        >{t("agent.htmlApp.open")}</span
      >
    {/if}
  </button>
{:else}
  <div
    class="flex items-center gap-2 rounded-lg border border-[var(--hairline)] bg-base-100 px-3 py-2 text-sm text-base-content/50"
  >
    <div
      class="h-3 w-3 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
    ></div>
    <span>{t("agent.htmlApp.generating")}</span>
  </div>
{/if}
