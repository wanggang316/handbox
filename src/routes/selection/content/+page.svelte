<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Eye, Languages, Sparkles, X, Pin, PinOff, Copy, RotateCcw, MessageCirclePlus, ChevronDown, Loader2 } from "@lucide/svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { hideContentPanel, setContentPanelPinned } from "$lib/api/selection";
  import { settingsState } from "$lib/states/settings.svelte";
  import * as agentApi from "$lib/api/agent";
  import * as chatApi from "$lib/api/chat";
  import { translateWordStream, recordLookup, listWords } from "$lib/api/word";
  import type { TranslateWordResponse } from "$lib/types";

  const appWindow = getCurrentWindow();

  // 内容状态
  let content = $state({
    mode: "" as "show" | "translate" | "ai" | "",
    text: "",
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  // 翻译状态
  let translation = $state({
    isLoading: false,
    result: null as TranslateWordResponse | null,
    error: null as string | null,
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

      // 如果是翻译模式，自动开始翻译
      if (mode === "translate" && text) {
        await handleTranslate();
      }

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

    // 如果切换到翻译模式且有文本，自动开始翻译
    if (newMode === "translate" && content.text) {
      await handleTranslate();
    }
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

  /**
   * 获取翻译 Session ID，如果没有则创建
   */
  async function getOrCreateTranslationSession(): Promise<string | null> {
    try {
      const settings = settingsState.settings;
      const translation = settings?.translation;
      const currentSessionId = translation?.sessionId;

      if (currentSessionId) {
        return currentSessionId;
      }

      // 没有 sessionId，返回 null
      return null;
    } catch (error) {
      console.error("Failed to get translation session:", error);
      return null;
    }
  }

  /**
   * 执行翻译
   */
  async function handleTranslate() {
    if (!content.text || translation.isLoading) return;

    const sessionId = await getOrCreateTranslationSession();
    if (!sessionId) {
      translation.error = "请先在单词本页面配置翻译 Agent 和模型";
      return;
    }

    translation.isLoading = true;
    translation.error = null;
    translation.result = null;

    try {
      await translateWordStream(sessionId, content.text, {
        onChunk: (text) => {
          translation.result = {
            term: content.text,
            translation: text,
            targetLanguage: "unknown",
            phonetic: null,
            explanation: null,
          };
        },
        onComplete: async (result) => {
          translation.result = result;

          // 保存到单词本查询历史
          try {
            await recordLookup({
              term: content.text,
              translation: result.translation,
              phonetic: result.phonetic,
              explanation: result.explanation,
              sourceLanguage: "auto",
              targetLanguage: result.targetLanguage,
            });
          } catch (error) {
            console.error("Failed to record lookup:", error);
          }
        },
        onError: (error) => {
          console.error("Translation failed:", error);
          translation.error = "翻译失败";
        },
      });
    } catch (error) {
      console.error("Translation error:", error);
      translation.error = "翻译失败";
    } finally {
      translation.isLoading = false;
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
    {#if content.mode === "translate"}
      <!-- 翻译模式 -->
      {#if translation.isLoading}
        <div class="flex items-center justify-center py-8">
          <Loader2 class="size-5 animate-spin text-primary" />
          <span class="ml-2 text-sm text-base-content/60">翻译中...</span>
        </div>
      {:else if translation.error}
        <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
          {translation.error}
        </div>
      {:else if translation.result}
        <div class="space-y-3">
          <!-- 原文 -->
          <div class="p-2 rounded-lg bg-base-200">
            <p class="text-xs text-base-content/50 mb-1">原文</p>
            <p class="text-sm text-base-content whitespace-pre-wrap break-words">
              {content.text}
            </p>
          </div>
          <!-- 译文 -->
          <div class="p-2 rounded-lg bg-base-100">
            <p class="text-xs text-base-content/50 mb-1">译文</p>
            <p class="text-sm text-base-content whitespace-pre-wrap break-words font-medium">
              {translation.result.translation}
            </p>
            {#if translation.result.phonetic}
              <p class="text-xs text-base-content/50 mt-1">
                [{translation.result.phonetic}]
              </p>
            {/if}
            {#if translation.result.explanation}
              <p class="text-xs text-base-content/70 mt-1">
                {translation.result.explanation}
              </p>
            {/if}
          </div>
        </div>
      {:else}
        <p class="text-sm text-base-content/40 text-center py-4">
          等待翻译...
        </p>
      {/if}
    {:else if content.text}
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
      {#if content.mode === "translate"}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleTranslate}
          title="重新翻译"
          disabled={!content.text || translation.isLoading}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {:else}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleRegenerate}
          title="重新生成"
          disabled={!content.text}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {/if}
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
