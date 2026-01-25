<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Eye, Languages, Sparkles, X, Pin, PinOff, Copy, RotateCcw, MessageCirclePlus, ChevronDown } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
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

  // 下拉框状态
  let showModeDropdown = $state(false);

  // 模式配置
  const modeConfig = {
    show: { icon: Eye, label: "显示", color: "text-red-600" },
    translate: { icon: Languages, label: "翻译", color: "text-blue-600" },
    ai: { icon: Sparkles, label: "问 AI", color: "text-purple-600" },
  };

  onMount(() => {
    console.log("=====> [selection/content] onMount executed");

    // 重置下拉框状态
    showModeDropdown = false;

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

  // 复制文本
  async function handleCopy() {
    await writeText(content.text);
  }

  // 重新生成
  async function handleRegenerate() {
    // TODO: 触发重新生成逻辑
    console.log("重新生成:", content.mode);
  }

  // 继续问
  async function handleContinue() {
    // TODO: 触发继续问逻辑
    console.log("继续问");
  }

  // 切换模式
  async function handleModeChange(newMode: "show" | "translate" | "ai") {
    content.mode = newMode;
    showModeDropdown = false;
    // TODO: 触发模式切换逻辑，重新生成内容
    console.log("模式切换为:", newMode);
  }

  // 切换下拉框显示状态
  function toggleDropdown() {
    showModeDropdown = !showModeDropdown;
  }

  // 点击外部关闭下拉框
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".mode-dropdown")) {
      showModeDropdown = false;
    }
  }
</script>

<svelte:window onclick={handleClickOutside} />

<div class="flex flex-col w-full h-full bg-base-100 rounded-2xl shadow-lg overflow-hidden">
  <!-- 标题栏 -->
  {#if content.mode && modeConfig[content.mode]}
    {@const config = modeConfig[content.mode]}
    <div class="flex items-center justify-between px-3 py-2 border-b border-base-300 cursor-move" data-tauri-drag-region>
      <!-- 模式下拉框 -->
      <div class="mode-dropdown relative">
        <button
          class="flex items-center gap-1.5 px-2 py-1.5 rounded-lg hover:bg-base-200 transition-colors {config.color}"
          onclick={toggleDropdown}
        >
          <config.icon class="size-4" />
          <span class="text-sm font-medium">{config.label}</span>
          <ChevronDown class="size-3.5 opacity-60" />
        </button>

        <!-- 下拉菜单 -->
        {#if showModeDropdown}
          <div class="absolute top-full left-0 mt-1 bg-base-100 rounded-lg shadow-lg border border-base-300 py-1 min-w-[120px] z-50">
            {#each Object.entries(modeConfig) as [key, value]}
              {@const isActive = key === content.mode}
              <button
                class="flex items-center gap-2 w-full px-3 py-2 text-sm hover:bg-base-200 transition-colors {isActive ? 'bg-base-300' : ''}"
                class:text-primary={isActive}
                class:text-base-content={!isActive}
                onclick={() => handleModeChange(key as "show" | "translate" | "ai")}
              >
                <value.icon class="size-4" />
                <span>{value.label}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-200 transition-colors {isPinned ? 'text-primary' : 'text-base-content/50 hover:text-base-content'}"
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
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-200 text-base-content/50 hover:text-base-content transition-colors"
          onclick={handleClose}
        >
          <X class="size-4" />
        </button>
      </div>
    </div>
  {/if}

  <!-- 内容区域 -->
  <div class="flex-1 p-3 overflow-auto min-h-0">
    {#if content.text}
      <p class="text-sm text-base-content whitespace-pre-wrap break-words leading-relaxed">
        {content.text}
      </p>
    {:else}
      <p class="text-sm text-base-content/40 text-center py-4">暂无内容</p>
    {/if}
  </div>

  <!-- 底部按钮区域 -->
  <div class="flex items-center justify-between px-3 py-1.5 border-t border-base-300 bg-base-200/50">
    <!-- 左下角：复制、重新生成 -->
    <div class="flex items-center gap-1">
      <button
        class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
        onclick={handleCopy}
        title="复制"
        disabled={!content.text}
      >
        <Copy class="size-3.5" />
      </button>
      <button
        class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
        onclick={handleRegenerate}
        title="重新生成"
        disabled={!content.text}
      >
        <RotateCcw class="size-3.5" />
      </button>
    </div>

    <!-- 右下角：继续问 -->
    <button
      class="flex items-center px-2 py-1 text-xs font-medium text-primary hover:bg-primary/10 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
      onclick={handleContinue}
      title="继续问"
      disabled={!content.text}
    >
      <MessageCirclePlus class="size-3.5" />
      <span>继续问</span>
    </button>
  </div>
</div>
