<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Copy, Languages, Sparkles, EllipsisVertical } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    hideMenuPanel,
    showContentPanel,
    showSettingsPanel,
    type ContentPanelMode,
  } from "$lib/api/selection";
  import { t } from "$lib/i18n";

  const appWindow = getCurrentWindow();

  let captured = $state({
    text: "",
    x: 0,
    y: 0,
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  onMount(() => {
    console.log("=====> [selection/menu] onMount executed");
    // Global text-selection signal from the backend
    const unlisten = listen("global-selection", async (event: any) => {
      const { text, x, y, app_info } = event.payload;

      captured = { text, x, y, app_info };
      console.log("-----> captured: ", captured);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // Hide via a backend command so panel state stays in sync
  async function hidePanel() {
    await hideMenuPanel();
  }

  async function openContentPanel(mode: ContentPanelMode) {
    await showContentPanel(mode, {
      text: captured.text,
      x: captured.x,
      y: captured.y,
      app_info: captured.app_info,
    });
    await hidePanel();
  }

  async function handleCopy() {
    await writeText(captured.text);
    await hidePanel();
  }

  async function handleTranslate() {
    await openContentPanel("translate");
  }

  async function handleAi() {
    await openContentPanel("ai");
  }

  async function handleSettings() {
    const [position, size, scale] = await Promise.all([
      appWindow.outerPosition(),
      appWindow.outerSize(),
      appWindow.scaleFactor(),
    ]);
    const logicalX = position.x / scale;
    const logicalY = position.y / scale;
    const logicalWidth = size.width / scale;
    const logicalHeight = size.height / scale;
    const x = logicalX + logicalWidth - 40;
    const y = logicalY + logicalHeight + 8;
    await showSettingsPanel(x, y);
  }
</script>

{#if captured.text}
<div
  class="flex items-center w-full h-full p-1 bg-[var(--bg-card)] rounded-xl shadow-lg border border-[var(--hairline)] overflow-hidden"
>
  <div
    class="flex flex-row flex-1 items-center justify-between gap-1 px-2 text-[14px] text-base-content"
  >
    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-base-200 hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleCopy}
    >
      <Copy class="size-3.5" />
      {t("common.copy")}
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-base-200 hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleTranslate}
    >
      <Languages class="size-3.5" />
      {t("selection.modeTranslate")}
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-base-200 hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
      onclick={handleAi}
    >
      <Sparkles class="size-3.5" />
      {t("selection.modeAi")}
    </button>
  </div>
  <button
    class="flex items-center justify-center flex-none w-8 h-8 rounded-full bg-base-200 hover:bg-base-300 transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]"
    onclick={handleSettings}
  >
    <EllipsisVertical class="size-3.5" />
  </button>
</div>
{/if}
