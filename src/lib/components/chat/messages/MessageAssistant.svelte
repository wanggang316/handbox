<script lang="ts">
  import {
    Copy,
    RotateCcw,
    Trash2,
    ChevronDown,
    ChevronRight,
    X as CloseIcon,
    Save,
    FolderOpen,
    Star,
  } from "@lucide/svelte";
  import ToolCallList from "./ToolCallCard.svelte";
  import type { Message, MessageAttachment } from "$lib/types";
  import type { TextRange } from "$lib/types/favorite";
  import { messageStore, favoriteStore } from "$lib/states";
  import { highlightRange, openInBrowser, renderMarkdown, markdownInteractions, copyToClipboard } from "$lib/utils";
  import {
    resolveLocalAssetPath,
    isTauriEnvironment,
    openPathInSystem,
  } from "$lib/utils/tauri";
  import FavoriteButton from "$lib/components/favorite/FavoriteButton.svelte";
  import TextSelectionMenu from "$lib/components/favorite/TextSelectionMenu.svelte";

  interface Props {
    message?: Message;
    isOperating?: boolean;
    isStreaming?: boolean;
    isReasoning?: boolean;
    isMessageLoading?: boolean;
    onCopy?: (content: string) => void;
    onRegenerate?: (messageId: string) => void;
    onDelete?: (messageId: string) => void;
  }

  let {
    message,
    isOperating = false,
    isStreaming = false,
    isReasoning = false,
    isMessageLoading = false,
    onCopy,
    onRegenerate,
    onDelete,
  }: Props = $props();

  // reasoning 折叠状态，流式消息默认收起，完成的消息默认展开
  let reasoningExpanded = $state(!isStreaming);

  // 获取provider配置
  const providerConfig = $derived(() => {
    if (message?.config?.providerId) {
      return messageStore.getProviderConfig(message.config.providerId);
    }
    return undefined;
  });

  const textRanges = $derived.by(() => {
    if (!message?.id || !message.sessionId) return [];
    const ranges = favoriteStore.textRangesByMessageId[message.id] ?? [];
    return ranges.map((range) => ({ start: range.start, end: range.end }));
  });

  let showRangeMenu = $state(false);
  let rangeMenuX = $state(0);
  let rangeMenuY = $state(0);
  let hoveredRange = $state<TextRange | null>(null);
  let isRangeMenuHovering = $state(false);

  function handleRangeHover(payload: { range: TextRange; rect: DOMRect }) {
    hoveredRange = payload.range;
    rangeMenuX = payload.rect.left + payload.rect.width / 2;
    rangeMenuY = payload.rect.top - 8;
    showRangeMenu = true;
  }

  function handleRangeLeave() {
    if (isRangeMenuHovering || showRangeMenu) return;
    showRangeMenu = false;
    hoveredRange = null;
  }

  async function handleRemoveRange() {
    if (!hoveredRange || !message?.id || !message.sessionId) return;
    try {
      await favoriteStore.removeTextRange(
        message.id,
        message.sessionId,
        hoveredRange,
        message.role,
        message.content,
      );
    } catch (error) {
      console.error("Failed to remove text favorite range:", error);
    } finally {
      showRangeMenu = false;
      hoveredRange = null;
    }
  }

  function handleRangeMenuOutside(event: MouseEvent) {
    if (!showRangeMenu) return;
    const target = event.target as HTMLElement;
    if (!target.closest(".favorite-range-menu")) {
      showRangeMenu = false;
      hoveredRange = null;
    }
  }

  let assets = $state<MessageAttachment[]>([]);
  let isAssetsLoading = $state(false);
  $effect(() => {
    const newAssets = message?.generatedAssets ?? [];
    assets = newAssets;
    // 只有当明确知道正在生成资源时才显示加载状态
    isAssetsLoading = Boolean(
      isStreaming &&
        messageStore.streamingIsGeneratingAssets
    );
  });

  // 右键菜单状态
  let contextMenu = $state<{
    show: boolean;
    x: number;
    y: number;
    asset: MessageAttachment | null;
  }>({
    show: false,
    x: 0,
    y: 0,
    asset: null,
  });

  // 收藏图片状态
  let isFavoritingImage = $state(false);

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  // 格式化持续时间
  function formatDuration(duration?: number): string {
    if (!duration) return "";

    if (duration < 1000) {
      return `${duration}ms`;
    } else {
      return `${(duration / 1000).toFixed(1)}s`;
    }
  }

  // 处理操作
  async function handleCopy() {
    if (message?.content) {
      if (onCopy) {
        onCopy(message.content);
      } else {
        await copyToClipboard(message.content);
      }
    }
  }

  function handleRegenerate() {
    if (message?.id) {
      onRegenerate?.(message.id);
    }
  }

  function handleDelete() {
    if (message?.id) {
      onDelete?.(message.id);
    }
  }

  // 切换推理过程显示状态
  function toggleReasoning() {
    reasoningExpanded = !reasoningExpanded;
  }

  const assetUrl = (path?: string) => resolveLocalAssetPath(path);

  async function openAssetExternally(asset: MessageAttachment) {
    const path = asset.path || "";
    if (!path) {
      console.warn("[MessageAssistant] No valid path to open", asset);
      return;
    }

    if (
      path.startsWith("http://") ||
      path.startsWith("https://") ||
      path.startsWith("data:")
    ) {
      await openInBrowser(path);
      return;
    }

    await openPathInSystem(path);
  }

  // 处理右键菜单
  function handleContextMenu(event: MouseEvent, asset: MessageAttachment) {
    event.preventDefault();

    // 先移除旧的监听器（如果有）
    if (contextMenu.show) {
      document.removeEventListener('click', handleClickOutsideRef);
    }

    contextMenu = {
      show: true,
      x: event.clientX,
      y: event.clientY,
      asset,
    };

    // 延迟添加点击外部监听器
    setTimeout(() => {
      document.addEventListener('click', handleClickOutsideRef);
    }, 0);
  }

  function closeContextMenu() {
    contextMenu = { show: false, x: 0, y: 0, asset: null };
    document.removeEventListener('click', handleClickOutsideRef);
  }

  // 创建稳定的引用函数
  function handleClickOutsideRef(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest('.context-menu')) {
      closeContextMenu();
    }
  }

  // 右键菜单操作
  async function copyImage() {
    if (!contextMenu.asset?.path) return;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("clipboard_copy_image", { path: contextMenu.asset.path });
      console.log("[MessageAssistant] Image copied to clipboard");
    } catch (error) {
      console.error("[MessageAssistant] Failed to copy image", error);
    }
    closeContextMenu();
  }

  async function saveImage() {
    if (!contextMenu.asset?.path) return;

    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { copyFile } = await import("@tauri-apps/plugin-fs");

      const savePath = await save({
        defaultPath: contextMenu.asset.name,
        filters: [
          {
            name: "Images",
            extensions: ["png", "jpg", "jpeg", "gif", "webp"],
          },
        ],
      });

      if (savePath) {
        await copyFile(contextMenu.asset.path, savePath);
        console.log("[MessageAssistant] Image saved to", savePath);
      }
    } catch (error) {
      console.error("[MessageAssistant] Failed to save image", error);
    }
    closeContextMenu();
  }

  async function showInFinder() {
    if (!contextMenu.asset?.path) return;

    try {
      const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
      await revealItemInDir(contextMenu.asset.path);
      console.log("[MessageAssistant] Revealed item in Finder");
    } catch (error) {
      console.error("[MessageAssistant] Failed to show in Finder", error);
    }
    closeContextMenu();
  }

  async function favoriteImage() {
    if (!contextMenu.asset || !message) return;

    isFavoritingImage = true;
    try {
      const imageMarkdown = `![${contextMenu.asset.name}](${contextMenu.asset.path})`;
      await favoriteStore.toggleFavorite(
        message.id ?? "",
        message.sessionId,
        imageMarkdown,
        message.role ?? "assistant",
        "image",
        [],
        undefined,
        undefined
      );
      closeContextMenu();
    } catch (error) {
      console.error("[MessageAssistant] Failed to favorite image", error);
    } finally {
      isFavoritingImage = false;
    }
  }
</script>

<div
  class="group relative"
  id={message?.id ? "message-" + message.id : undefined}
>
  <!-- 消息容器 -->
  <div class="flex flex-col gap-2">
    <!-- 模型供应商图标（模型） -->
    <div class="flex flex-row gap-2">
      <div
        class="w-8 h-8 rounded-full bg-base-300 flex items-center justify-center"
      >
        <img
          src={providerConfig()?.icon}
          alt={providerConfig()?.type_name || "AI"}
          class="w-4 h-4 object-contain"
        />
      </div>

      {#if message?.config?.modelId}
        <div class="flex items-center gap-1 text-base-content/60 text-xs">
          {message.config.modelId}
        </div>
      {/if}
    </div>

    <!-- 消息内容 -->
    <div class="flex-1 min-w-0">
      {#if isMessageLoading}
        <!-- 加载状态 -->
        <div class="max-w-full py-2 text-base-content flex items-center">
          <div
            class="h-4 w-4 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
          ></div>
        </div>
      {:else}
        <!-- 消息气泡 -->
        <div class="max-w-full py-0 text-base-content">
          <!-- 推理过程（如果有） -->
          {#if message?.reasoning}
            <div class="mb-4">
              <!-- 推理过程标题，可点击折叠 -->
              <button
                class="flex items-center gap-1 my-2 text-left hover:bg-base-300 rounded-full py-1 px-2"
                onclick={toggleReasoning}
              >
                {#if reasoningExpanded}
                  <ChevronDown size={16} class="text-base-content" />
                {:else}
                  <ChevronRight size={16} class="text-base-content" />
                {/if}
                <span class="text-sm font-medium text-base-content/80">
                  {isReasoning ? "推理中..." : "推理过程"}
                </span>
              </button>

              <!-- 推理过程内容，根据展开状态显示 -->
              {#if reasoningExpanded}
                <div
                  class="mt-2 mb-6 px-4 text-sm border-l border-[var(--hairline)] text-base-content/80 break-words leading-relaxed reasoning-content markdown-content"
                  use:markdownInteractions
                >
                  {@html renderMarkdown(message.reasoning)}
                </div>
              {/if}
            </div>
          {/if}

  <!-- 消息内容 -->
          {#if message && message.id && message.sessionId}
            <TextSelectionMenu
              messageId={message.id}
              chatId={message.sessionId}
              content={message.content}
              role={message.role}
            >
              <div
                class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
                data-message-id={message.id}
                use:highlightRange={{
                  ranges: textRanges,
                  onRangeHover: handleRangeHover,
                  onRangeLeave: handleRangeLeave,
                  hoverDelayMs: 2000,
                  version: favoriteStore.textRangesVersion,
                }}
                data-favorite-highlight-version={favoriteStore.textRangesVersion}
                use:markdownInteractions
              >
                {@html renderMarkdown(message.content || "")}
              </div>
            </TextSelectionMenu>
          {:else if message}
            <div
              class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
              use:markdownInteractions
            >
              {@html renderMarkdown(message.content || "")}
            </div>
          {/if}

          {#if isAssetsLoading}
            <div
              class="mt-4 flex items-center gap-3 rounded-lg border border-dashed border-[var(--hairline)] px-4 py-3 text-sm text-base-content/70"
            >
              <div
                class="w-4 h-4 border-2 border-base-content/30 border-t-transparent rounded-full animate-spin"
              ></div>
              <span>图像生成中…</span>
            </div>
          {/if}

          {#if assets?.length}
            <div class="mt-4 flex flex-wrap gap-4">
              {#each assets as asset (asset.id)}
                <div
                  class="relative rounded-lg bg-base-100 max-w-[320px]"
                  title="点击在系统预览中打开"
                  role="button"
                  tabindex="0"
                  onclick={() => openAssetExternally(asset)}
                  oncontextmenu={(e) => handleContextMenu(e, asset)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      openAssetExternally(asset);
                    }
                  }}
                >
                  <img
                    src={assetUrl(asset.path)}
                    alt={asset.name}
                    class="w-full h-auto max-w-[320px] object-contain rounded-md"
                  />
                </div>
              {/each}
            </div>
          {/if}

          <!-- 工具调用记录 -->
          {#if message?.toolCalls?.length}
            <ToolCallList
              toolCalls={message?.toolCalls ?? []}
              messageId={message?.id}
              {isStreaming}
            />
          {/if}

          {#if !isStreaming && !isMessageLoading}
            <!-- 性能信息 -->
            <!-- <div class="flex flex-row gap-2 mt-6 text-xs text-base-content/60">
            {#if message?.createdAt}
              <span>
                {formatTime(message.createdAt)}
              </span>
            {/if}
            {#if message?.inputTokens || message?.outputTokens || message?.totalTokens}
              <span class="font-medium">Token:</span>
              <span>
                {#if message.inputTokens}
                  | 输入: {message.inputTokens}{/if}
                {#if message.outputTokens}
                  | 输出: {message.outputTokens}{/if}
                {#if message.totalTokens}
                  | 总计: {message.totalTokens}{/if}
              </span>
            {/if}

            {#if message?.duration}
              <span> | 耗时: {formatDuration(message.duration)}</span>
            {/if}
          </div> -->
          {/if}
        </div>

        <!-- 消息操作按钮 (仅在非流式且非加载状态下显示) -->
        {#if !isStreaming && !isMessageLoading}
          <div
            class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200"
          >
            <div class="inline-flex gap-1">
              <!-- 收藏按钮 -->
              {#if message && message.id}
                <FavoriteButton
                  messageId={message.id}
                  chatId={message.sessionId}
                  content={message.content}
                  role={message.role}
                />
              {/if}

              <!-- 复制按钮 -->
              <button
                class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors"
                title="复制消息"
                onclick={handleCopy}
              >
                <Copy class="w-3.5 h-3.5" />
              </button>

              <!-- 重新生成按钮 -->
              <button
                class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="重新生成"
                disabled={isOperating}
                onclick={handleRegenerate}
              >
                {#if isOperating}
                  <div
                    class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"
                  ></div>
                {:else}
                  <RotateCcw class="w-3.5 h-3.5" />
                {/if}
              </button>

              <!-- 删除按钮 -->
              <button
                class="p-1.5 text-base-content/60 hover:text-error hover:bg-error/10 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                title="删除消息"
                disabled={isOperating}
                onclick={handleDelete}
              >
                {#if isOperating}
                  <div
                    class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"
                  ></div>
                {:else}
                  <Trash2 class="w-3.5 h-3.5" />
                {/if}
              </button>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>

  <!-- 右键菜单 -->
{#if contextMenu.show}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    role="menu"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => {
      if (e.key === 'Escape') {
        closeContextMenu();
      }
    }}
  >
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={copyImage}
    >
      <Copy size={14} />
      <span>复制图片</span>
    </button>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={saveImage}
    >
      <Save size={14} />
      <span>保存图片</span>
    </button>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={favoriteImage}
      disabled={isFavoritingImage}
    >
      {#if isFavoritingImage}
        <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
      {:else}
        <Star size={14} />
      {/if}
      <span>收藏图片</span>
    </button>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={showInFinder}
    >
      <FolderOpen size={14} />
      <span>在 Finder 中打开</span>
    </button>
  </div>
{/if}

{#if showRangeMenu && hoveredRange}
  <div
    class="favorite-range-menu fixed z-[10040] bg-base-100 border border-base-300 rounded-lg shadow-lg px-2 py-1 text-xs"
    style="left: {rangeMenuX}px; top: {rangeMenuY}px; transform: translateX(-50%);"
    role="menu"
    tabindex="-1"
    aria-label="收藏范围操作"
    onmouseenter={() => (isRangeMenuHovering = true)}
    onmouseleave={() => {
      isRangeMenuHovering = false;
      showRangeMenu = false;
      hoveredRange = null;
    }}
  >
    <button
      class="px-2 py-1 rounded hover:bg-error/10 text-error"
      onclick={handleRemoveRange}
    >
      取消收藏
    </button>
  </div>
{/if}

<svelte:window on:click={handleRangeMenuOutside} />
