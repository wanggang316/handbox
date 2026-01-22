<script lang="ts">
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  // import { writeText } from "@tauri-plugin-clipboard-manager";

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

  onMount(() => {
    // 监听后端发送的全局划词信号
    const unlisten = listen("global-selection", async (event: any) => {
      const { text, x, y, app_info } = event.payload;

      captured = { text, x, y, app_info };
      console.log("-----> captured: ", captured);
      visible = true;

      // 1. 计算位置：出现在鼠标上方 48 像素处
      // 使用 LogicalPosition 自动处理 Retina 屏和多屏缩放
      await appWindow.setPosition(new LogicalPosition(x, y - 48));

      // 2. 显示窗口并置顶
      await appWindow.show();
      await appWindow.setFocus();
    });

    // 组件销毁时取消监听
    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // 处理点击动作
  async function handleAction(mode: "copy" | "translate" | "ai") {
    visible = false;
    await appWindow.hide();

    if (mode === "copy") {
      // await writeText(captured.text);
    } else {
      // 1. 唤起内容大面板
      await invoke("show_content_window");
      // 2. 发送初始化信号给内容面板 (让它开始执行 AI/翻译逻辑)
      await emit("init-content", {
        mode,
        text: captured.text,
        app_info: captured.app_info,
      });
    }
  }
</script>

{#if visible && captured.text}
  <div
    class="flex items-center h-10 px-1 bg-white/90 backdrop-blur-xl border border-gray-200/50 rounded-full shadow-2xl select-none overflow-hidden animate-in fade-in zoom-in duration-150"
  >
    <div
      class="flex items-center pl-3 pr-2 border-r border-gray-200/60 max-w-[100px]"
    >
      <span
        class="text-[10px] font-medium text-gray-500 truncate"
        title={captured.app_info.bundle_id}
      >
        {captured.app_info.name}
      </span>
    </div>

    <div class="px-3 max-w-[120px] truncate">
      <span class="text-xs font-semibold text-gray-800">
        {captured.text}
      </span>
    </div>

    <div class="flex items-center gap-0.5 pr-1">
      <button class="btn-action" onclick={() => handleAction("copy")}>
        复制
      </button>

      <button
        class="btn-action text-blue-600 hover:bg-blue-50"
        onclick={() => handleAction("translate")}
      >
        翻译
      </button>

      <button
        class="btn-action text-purple-600 hover:bg-purple-50"
        onclick={() => handleAction("ai")}
      >
        AI
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
