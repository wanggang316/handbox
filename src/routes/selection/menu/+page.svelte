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

  // Svelte 5 响应式状态
  let captured = $state({
    text: "",
    x: 0,
    y: 0,
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  onMount(() => {
    console.log("=====> [selection/menu] onMount executed");
    // 监听后端发送的全局划词信号
    const unlisten = listen("global-selection", async (event: any) => {
      const { text, x, y, app_info } = event.payload;

      captured = { text, x, y, app_info };
      console.log("-----> captured: ", captured);
    });

    // 组件销毁时取消监听
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // 隐藏面板（通过后端命令，确保状态同步）
  async function hidePanel() {
    await hideMenuPanel();
  }

  // 显示内容面板的通用方法
  async function openContentPanel(mode: ContentPanelMode) {
    await showContentPanel(mode, {
      text: captured.text,
      x: captured.x,
      y: captured.y,
      app_info: captured.app_info,
    });
    await hidePanel();
  }

  // 复制文本
  async function handleCopy() {
    await writeText(captured.text);
    await hidePanel();
  }

  // 翻译
  async function handleTranslate() {
    await openContentPanel("translate");
  }

  // 问 AI
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
<div class="flex items-center w-full h-full p-1 bg-white">
  <div
    class="flex flex-row flex-1 items-center justify-between gap-1 px-2 text-[14px] text-gray-600"
  >
    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-gray-100"
      onclick={handleCopy}
    >
      <Copy class="size-3.5" />
      {t("common.copy")}
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-gray-100"
      onclick={handleTranslate}
    >
      <Languages class="size-3.5" />
      {t("selection.modeTranslate")}
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg hover:bg-gray-100"
      onclick={handleAi}
    >
      <Sparkles class="size-3.5" />
      {t("selection.modeAi")}
    </button>
  </div>
  <button
    class="flex items-center justify-center flex-none w-8 h-8 rounded-full bg-gray-200"
    onclick={handleSettings}
  >
    <EllipsisVertical class="size-3.5" />
  </button>
</div>
{/if}
