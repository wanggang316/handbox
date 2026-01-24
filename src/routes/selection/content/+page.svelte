<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Eye, Languages, Sparkles, X, Pin, PinOff } from "@lucide/svelte";
  import { hideContentPanel, setContentPanelPinned } from "$lib/api/selection";

  const appWindow = getCurrentWindow();

  // 内容状态
  let content = $state({
    mode: "" as "show" | "translate" | "ai" | "",
    text: "",
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  // 置顶状态
  let isPinned = $state(false);

  // 模式配置
  const modeConfig = {
    show: { icon: Eye, label: "显示", color: "text-red-600" },
    translate: { icon: Languages, label: "翻译", color: "text-blue-600" },
    ai: { icon: Sparkles, label: "问 AI", color: "text-purple-600" },
  };

  onMount(() => {
    console.log("=====> [selection/content] onMount executed");

    // 监听 init-content 事件
    const unlisten = listen("init-content", async (event: any) => {
      const { mode, text, x, y, app_info } = event.payload;
      content = { mode, text, app_info };
      // 新内容时重置置顶状态
      isPinned = false;
      await setContentPanelPinned(false);
      console.log("-----> content received: ", content);

      // // 设置位置：x 居中，y 在选中文字下方
      // await appWindow.setPosition(new LogicalPosition(x - 160, y + 8));
      // await appWindow.show();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // 关闭面板
  async function handleClose() {
    content = { mode: "", text: "", app_info: { name: "", bundle_id: "", pid: 0 } };
    isPinned = false;
    await hideContentPanel();
  }

  // 切换置顶状态
  async function togglePin() {
    isPinned = !isPinned;
    await setContentPanelPinned(isPinned);
  }
</script>

<div class="flex flex-col w-full h-full bg-white rounded-2xl shadow-lg overflow-hidden">
  <!-- 标题栏 -->
  {#if content.mode && modeConfig[content.mode]}
    {@const config = modeConfig[content.mode]}
    <div class="flex items-center justify-between px-3 py-2 border-b border-gray-100">
      <div class="flex items-center gap-2 {config.color}">
        <config.icon class="size-4" />
        <span class="text-sm font-medium">{config.label}</span>
      </div>
      <div class="flex items-center gap-1">
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-gray-100 transition-colors {isPinned ? 'text-blue-500' : 'text-gray-400 hover:text-gray-600'}"
          onclick={togglePin}
          title={isPinned ? "取消置顶" : "置顶"}
        >
          {#if isPinned}
            <Pin class="size-3.5" />
          {:else}
            <PinOff class="size-3.5" />
          {/if}
        </button>
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-gray-100 text-gray-400 hover:text-gray-600 transition-colors"
          onclick={handleClose}
        >
          <X class="size-4" />
        </button>
      </div>
    </div>
  {/if}

  <!-- 内容区域 -->
  <div class="flex-1 p-3 overflow-auto">
    {#if content.text}
      <p class="text-sm text-gray-700 whitespace-pre-wrap break-words leading-relaxed">
        {content.text}
      </p>
    {:else}
      <p class="text-sm text-gray-400 text-center py-4">暂无内容</p>
    {/if}
  </div>
</div>
