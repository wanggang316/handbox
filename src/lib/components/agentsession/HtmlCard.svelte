<script lang="ts">
  // Inline HTML card for `render_card` toolcall blocks (feat/html-segment-render).
  //
  // Render-only presentation: unlike AgentToolCallCard there is no expandable
  // Request/Response view — the card IS the content. Args become available at
  // tool_execution_start, so a valid parse renders immediately even while the
  // status is still `executing`; the skeleton only covers the brief window
  // before args are parseable, and errors get a compact error box.
  import { XCircle } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import type { ToolCallView } from "$lib/states/agentRun.svelte";
  import SandboxHost from "$lib/components/sandbox/SandboxHost.svelte";
  import { parseRenderCardArgs } from "./renderCard";

  interface Props {
    toolCall: ToolCallView;
  }

  let { toolCall }: Props = $props();

  const parsed = $derived(parseRenderCardArgs(toolCall.args));
</script>

{#if parsed && toolCall.status !== "error"}
  <div
    class="overflow-hidden rounded-lg border border-[var(--hairline)] bg-base-100"
  >
    {#if parsed.title}
      <div
        class="border-b border-[var(--hairline)] px-3 py-1.5 text-xs text-base-content/60"
      >
        {parsed.title}
      </div>
    {/if}
    <SandboxHost html={parsed.html} title={parsed.title ?? t("agent.htmlCard.iframeTitle")} />
  </div>
{:else if toolCall.status === "executing"}
  <div
    class="flex items-center gap-2 rounded-lg border border-[var(--hairline)] bg-base-100 px-3 py-2 text-sm text-base-content/50"
  >
    <div
      class="h-3 w-3 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
    ></div>
    <span>{t("agent.htmlCard.rendering")}</span>
  </div>
{:else}
  <div
    class="flex items-center gap-2 rounded-lg border border-error/40 bg-error/10 px-3 py-2 text-sm text-error"
  >
    <XCircle size={14} class="shrink-0" />
    <span>{t("agent.htmlCard.error")}</span>
  </div>
{/if}
