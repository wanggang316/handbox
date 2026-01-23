<script lang="ts">
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Eye, Copy, Languages, Sparkles, MoreVertical } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";

  const appWindow = getCurrentWindow();

  // Svelte 5 响应式状态
  let captured = $state({
    text: "",
    x: 0,
    y: 0,
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  // 记录显示状态，用于动画控制
  let visible = $state(false);

  // 手动追踪 hover 状态（解决 NSPanel hover 状态残留问题）
  let hoveredBtn = $state<string | null>(null);

  onMount(() => {
    // 监听后端发送的全局划词信号
    const unlisten = listen("global-selection", async (event: any) => {
      const { text, x, y, app_info } = event.payload;

      captured = { text, x, y, app_info };
      console.log("-----> captured: ", captured);
      visible = true;
      hoveredBtn = null; // 重置 hover 状态

      // 1. 计算位置：出现在鼠标上方 48 像素处
      // 使用 LogicalPosition 自动处理 Retina 屏和多屏缩放
      await appWindow.setPosition(new LogicalPosition(x - 180, y - 72));

      // 2. 显示窗口并置顶
      // await appWindow.show();
      // await appWindow.setFocus();
    });

    // 组件销毁时取消监听
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // 隐藏面板
  async function hidePanel() {
    visible = false;
    await appWindow.hide();
  }

  // 显示完整内容
  async function handleShow() {
    console.log("-----> show: ", captured.text);
    await hidePanel();
  }

  // 复制文本
  async function handleCopy() {
    await writeText(captured.text);
    await hidePanel();
  }

  // 翻译
  async function handleTranslate() {
    await hidePanel();
    await invoke("show_content_window");
    await emit("init-content", {
      mode: "translate",
      text: captured.text,
      app_info: captured.app_info,
    });
  }

  // 问 AI
  async function handleAi() {
    await hidePanel();
    await invoke("show_content_window");
    await emit("init-content", {
      mode: "ai",
      text: captured.text,
      app_info: captured.app_info,
    });
  }

  // 设置
  function handleSettings() {
    // TODO: 打开设置菜单
  }
</script>

{#if captured.text}
  <div
    class="flex items-center w-full h-full bg-white overflow-hidden"
  >
    <!-- 操作按钮组 -->
    <div
      class="flex items-center justify-center gap-1 px-2 w-full"
      onmouseleave={() => (hoveredBtn = null)}
    >
      <button
        class="flex items-center gap-1 px-2 py-1 text-[11px] font-medium text-gray-600 rounded-lg transition-all duration-150 active:scale-95 {hoveredBtn === 'show' ? 'bg-gray-100' : ''}"
        onclick={handleShow}
        onmouseenter={() => (hoveredBtn = "show")}
      >
        <Eye class="size-3.5" />
        显示
      </button>

      <button
        class="flex items-center gap-1 px-2 py-1 text-[11px] font-medium text-gray-600 rounded-lg transition-all duration-150 active:scale-95 {hoveredBtn === 'copy' ? 'bg-gray-100' : ''}"
        onclick={handleCopy}
        onmouseenter={() => (hoveredBtn = "copy")}
      >
        <Copy class="size-3.5" />
        复制
      </button>

      <button
        class="flex items-center gap-1 px-2 py-1 text-[11px] font-medium text-blue-600 rounded-lg transition-all duration-150 active:scale-95 {hoveredBtn === 'translate' ? 'bg-blue-50' : ''}"
        onclick={handleTranslate}
        onmouseenter={() => (hoveredBtn = "translate")}
      >
        <Languages class="size-3.5" />
        翻译
      </button>

      <button
        class="flex items-center gap-1 px-2 py-1 text-[11px] font-medium text-purple-600 rounded-lg transition-all duration-150 active:scale-95 {hoveredBtn === 'ai' ? 'bg-purple-50' : ''}"
        onclick={handleAi}
        onmouseenter={() => (hoveredBtn = "ai")}
      >
        <Sparkles class="size-3.5" />
        问 AI
      </button>

      <button
        class="flex items-center justify-center size-6 text-gray-500 rounded-full transition-all duration-150 active:scale-95 {hoveredBtn === 'settings' ? 'bg-gray-100' : ''}"
        onclick={handleSettings}
        onmouseenter={() => (hoveredBtn = "settings")}
      >
        <MoreVertical class="size-3.5" />
      </button>
    </div>
  </div>
{/if}

<!-- <style lang="postcss">
  /* 必须强制背景透明，否则 rounded-full 会有白色背景底色 */
  :global(html),
  :global(body) {
    background-color: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  .btn-action {
    @apply px-3 py-1.5 text-[11px] font-medium rounded-full transition-all active:scale-90 hover:bg-gray-100 text-gray-700;
  }
</style> -->
