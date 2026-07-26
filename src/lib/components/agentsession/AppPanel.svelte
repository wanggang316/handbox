<script lang="ts">
  // Right-side panel for the `render_app` artifact: live preview + source view.
  //
  // Preview and code stay mounted simultaneously with a CSS visibility switch
  // (mirroring hand-ai's html-artifact): unmounting the iframe on a toggle
  // would reset the running app's state. The iframe itself reloads only when
  // `artifact.content` changes (srcdoc is derived from it in SandboxHost).
  import { Code, Eye, X } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import SandboxHost from "$lib/components/sandbox/SandboxHost.svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import type { AppArtifact } from "./renderApp";

  interface Props {
    artifact: AppArtifact;
    onClose: () => void;
  }

  let { artifact, onClose }: Props = $props();

  let viewMode = $state<"preview" | "code">("preview");
</script>

<div
  class="flex w-[44%] min-w-[360px] max-w-[760px] shrink-0 flex-col border-l border-[var(--hairline)] bg-base-100"
>
  <!-- Header: title + preview/code toggle + close. -->
  <div
    class="flex shrink-0 items-center gap-2 border-b border-[var(--hairline)] px-3 py-2"
  >
    <span class="min-w-0 flex-1 truncate text-sm font-medium text-base-content">
      {artifact.title || t("agent.htmlApp.untitled")}
    </span>

    <div
      class="flex shrink-0 items-center rounded-md border border-[var(--hairline)] p-0.5"
    >
      <button
        type="button"
        class={`flex items-center gap-1 rounded px-2 py-0.5 text-xs transition-colors ${
          viewMode === "preview"
            ? "bg-base-200 text-base-content"
            : "text-base-content/50 hover:text-base-content"
        }`}
        onclick={() => (viewMode = "preview")}
      >
        <Eye size={12} />
        {t("agent.htmlApp.preview")}
      </button>
      <button
        type="button"
        class={`flex items-center gap-1 rounded px-2 py-0.5 text-xs transition-colors ${
          viewMode === "code"
            ? "bg-base-200 text-base-content"
            : "text-base-content/50 hover:text-base-content"
        }`}
        onclick={() => (viewMode = "code")}
      >
        <Code size={12} />
        {t("agent.htmlApp.code")}
      </button>
    </div>

    <button
      type="button"
      class="shrink-0 rounded p-1 text-base-content/50 transition-colors hover:bg-base-200 hover:text-base-content"
      onclick={onClose}
      title={t("agent.htmlApp.close")}
      aria-label={t("agent.htmlApp.close")}
    >
      <X size={14} />
    </button>
  </div>

  <!-- Body: both views stay mounted; CSS toggles visibility. -->
  <div class="relative min-h-0 flex-1">
    <div
      class="absolute inset-0"
      style:display={viewMode === "preview" ? "block" : "none"}
    >
      <SandboxHost
        html={artifact.content}
        mode="fill"
        title={artifact.title || t("agent.htmlApp.iframeTitle")}
      />
    </div>

    <div
      class="absolute inset-0 overflow-auto p-3 text-[13px]"
      style:display={viewMode === "code" ? "block" : "none"}
    >
      <!-- eslint-disable-next-line svelte/no-at-html-tags -- highlight.js output over escaped content -->
      {@html renderCodeBlock(artifact.content, { language: "html" })}
    </div>
  </div>
</div>
