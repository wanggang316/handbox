<script lang="ts">
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  // import { writeText } from "@tauri-plugin-clipboard-manager";

  const appWindow = getCurrentWindow();
  // Svelte 5 状态符文
  let capturedText = $state("");

  // 1. 监听来自后端的全局划词信号
  listen("global-selection", async (event: any) => {
    const { text, x, y } = event.payload;
    capturedText = text;

    console.log("-----> x: ", x, "y: ", y);
    // 移动窗口到鼠标上方 (y - 48 确保菜单在鼠标上方一点)
    // LogicalPosition 会自动处理屏幕缩放
    await appWindow.setPosition(new LogicalPosition(x, y - 48));
    // 显示并聚焦窗口
    await appWindow.show();
    await appWindow.setFocus();
  });

  // 2. 处理按钮点击动作
  async function handleAction(mode: "copy" | "translate" | "ai") {
    // 点击任何按钮后先隐藏当前菜单
    await appWindow.hide();

    if (mode === "copy") {
      // 调用 Tauri 剪切板插件
      // await writeText(capturedText);
    } else {
      // 调用 Rust 命令显示内容窗口 (需要在 Rust 端实现该命令)
      await invoke("show_content_window");
      // 发送初始化数据给内容面板
      await emit("init-content", { mode, text: capturedText });
    }
  }
</script>

<div class="fixed inset-0 bg-red-500 flex items-center justify-center">
  <span class="text-white">测试显示: {capturedText}</span>
</div>

<!-- <div
  class="inline-flex items-center h-10 px-2 bg-white/90 backdrop-blur-md border border-gray-200/50 rounded-full shadow-lg select-none"
>
  <button
    class="px-3 py-1 text-sm font-medium text-gray-700 hover:text-blue-600 transition-colors rounded-md hover:bg-gray-100/50 active:scale-95"
    onclick={() => handleAction("copy")}
  >
    复制
  </button>

  <div class="w-px h-4 bg-gray-300/70 mx-1"></div>

  <button
    class="px-3 py-1 text-sm font-medium text-gray-700 hover:text-blue-600 transition-colors rounded-md hover:bg-gray-100/50 active:scale-95"
    onclick={() => handleAction("translate")}
  >
    翻译
  </button>

  <div class="w-px h-4 bg-gray-300/70 mx-1"></div>

  <button
    class="px-3 py-1 text-sm font-medium text-gray-700 hover:text-blue-600 transition-colors rounded-md hover:bg-gray-100/50 active:scale-95"
    onclick={() => handleAction("ai")}
  >
    问 AI
  </button>
</div> -->
