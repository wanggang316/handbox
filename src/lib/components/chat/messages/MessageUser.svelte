<script lang="ts">
  import { RotateCcw, Copy, Pencil } from "@lucide/svelte";
  import type { Message } from "$lib/types";
  import type { TextRange } from "$lib/types/favorite";
  import { favoriteStore } from "$lib/states";
  import { highlightRange } from "$lib/utils";
  import { resolveLocalAssetPath, openPathInSystem } from "$lib/utils/tauri";
  import FavoriteButton from "$lib/components/favorite/FavoriteButton.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    message: Message;
    isOperating?: boolean;
    onCopy?: (content: string) => void;
    onResend?: (messageId: string) => void;
    onEdit?: (messageId: string, content: string) => void;
  }

  const assetUrl = (path?: string) => resolveLocalAssetPath(path);

  let {
    message,
    isOperating = false,
    onResend,
    onCopy,
    onEdit,
  }: Props = $props();

  const textRanges = $derived.by(() => {
    if (!message?.id || !message.chatId) return [];
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
    if (!hoveredRange || !message?.id || !message.chatId) return;
    try {
      await favoriteStore.removeTextRange(
        message.id,
        message.chatId,
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

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function handleCopy() {
    if (message?.content) {
      onCopy?.(message.content);
    }
  }

  function handleResend() {
    if (message?.id) {
      onResend?.(message.id);
    }
  }

  function handleEdit() {
    if (message?.id && message?.content) {
      onEdit?.(message.id, message.content);
    }
  }

  async function openAttachmentExternally(path?: string) {
    if (!path) {
      console.warn("[MessageUser] No valid path to open");
      return;
    }
    await openPathInSystem(path);
  }
</script>

<div class="group relative" id={"message-" + message.id}>
  <!-- 消息容器 -->
  <div class="flex justify-end">
    <!-- 消息内容 -->
    <div class="flex flex-col items-end">
         <!-- 消息气泡 -->
        <div
          class="inline-block max-w-full px-3.5 py-2 rounded-lg bg-base-200 text-base-content border border-[var(--hairline)]"
        >
          {#if message.id && message.chatId}
            <div
              class="whitespace-pre-wrap break-words text-[15px] leading-[1.6] text-left"
              use:highlightRange={{
                ranges: textRanges,
                onRangeHover: handleRangeHover,
                onRangeLeave: handleRangeLeave,
                hoverDelayMs: 2000,
                version: favoriteStore.textRangesVersion,
              }}
              data-favorite-highlight-version={favoriteStore.textRangesVersion}
            >
              {message.content}
            </div>
          {:else}
            <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6] text-left">
              {message.content}
            </div>
          {/if}
          {#if message.attachments?.length}
          <div class="mt-3 flex flex-wrap gap-3">
            {#each message.attachments as attachment}
              {#if attachment.mimeType?.startsWith("image/")}
                <div
                  class="relative rounded-lg max-w-[300px]"
                  title={t("chat.openInSystemPreview")}
                  role="button"
                  tabindex="0"
                  onclick={() => openAttachmentExternally(attachment.path)}
                  onkeydown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      openAttachmentExternally(attachment.path);
                    }
                  }}
                >
                  <img
                    src={assetUrl(attachment.path)}
                    alt={attachment.name}
                    class="w-full h-auto max-w-[300px] object-contain rounded-md"
                  />
                </div>
              {:else}
                <div
                  class="rounded-lg overflow-hidden border border-base-300 bg-base-100 p-2 max-w-[300px]"
                >
                  <div class="p-3 text-sm text-left">
                    <p class="font-medium">{attachment.name}</p>
                    <p class="text-xs text-base-content/60">
                      {attachment.mimeType}
                    </p>
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>

      <!-- 操作按钮 (hover显示) -->
      <div
        class="mt-2 gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex"
      >
        <!-- 收藏按钮 -->
        {#if message && message.id}
          <FavoriteButton
            messageId={message.id}
            chatId={message.chatId}
            content={message.content}
            role={message.role}
          />
        {/if}

        <!-- 复制按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors"
          title={t("chat.copyMessage")}
          onclick={handleCopy}
        >
          <Copy class="w-3.5 h-3.5" />
        </button>
        <!-- 编辑按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={t("chat.editAndResend")}
          disabled={isOperating}
          onclick={handleEdit}
        >
          <Pencil class="w-3.5 h-3.5" />
        </button>
        <!-- 重发按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title={t("chat.resendMessage")}
          disabled={isOperating}
          onclick={handleResend}
        >
          {#if isOperating}
            <div
              class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"
            ></div>
          {:else}
            <RotateCcw class="w-3.5 h-3.5" />
          {/if}
        </button>
      </div>
    </div>
  </div>
</div>

{#if showRangeMenu && hoveredRange}
  <div
    class="favorite-range-menu fixed z-[10040] bg-base-100 border border-base-300 rounded-lg shadow-lg px-2 py-1 text-xs"
    style="left: {rangeMenuX}px; top: {rangeMenuY}px; transform: translateX(-50%);"
    role="menu"
    tabindex="-1"
    aria-label={t("chat.favoriteRangeActions")}
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
      {t("chat.unfavorite")}
    </button>
  </div>
{/if}

<svelte:window on:click={handleRangeMenuOutside} />
