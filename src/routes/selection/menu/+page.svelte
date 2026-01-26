<script lang="ts">
  import { emit, listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import {
    Eye,
    Copy,
    Languages,
    Sparkles,
    EllipsisVertical,
  } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    hideMenuPanel,
    showContentPanel,
    type ContentPanelMode,
  } from "$lib/api/selection";

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

      // 1. 计算位置：出现在鼠标上方 48 像素处
      // 使用 LogicalPosition 自动处理 Retina 屏和多屏缩放
      // await appWindow.setPosition(new LogicalPosition(x - 180, y - 56));

      // 2. 显示窗口并置顶
      // await appWindow.show();
      // await appWindow.setFocus();
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

  // 显示完整内容
  async function handleShow() {
    console.log("-----> show: ", captured.text);
    await openContentPanel("show");
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

  // 设置
  function handleSettings() {
    // TODO: 打开设置菜单
  }
</script>

{#if captured.text}
<div class="flex items-center w-full h-full p-1 bg-white">
  <div
    class="flex flex-row flex-1 items-center justify-between gap-1 px-2 text-[14px] text-gray-600"
  >
    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-red-200 hover:bg-red-300"
      onclick={handleShow}
    >
      <Eye class="size-3.5" />
      显示
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-green-200"
      onclick={handleCopy}
    >
      <Copy class="size-3.5" />
      复制
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-blue-200"
      onclick={handleTranslate}
    >
      <Languages class="size-3.5" />
      翻译
    </button>

    <button
      class="flex items-center gap-1 px-2 py-1 rounded-lg bg-purple-200"
      onclick={handleAi}
    >
      <Sparkles class="size-3.5" />
      问 AI
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
