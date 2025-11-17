<script lang="ts">
  import { RotateCcw, Copy, Pencil } from "lucide-svelte";
  import type { Message } from "$lib/types";

  interface Props {
    message: Message;
    isOperating?: boolean;
    onCopy?: (content: string) => void;
    onResend?: (messageId: string) => void;
    onEdit?: (messageId: string, content: string) => void;
  }

  let { message, isOperating = false, onResend, onCopy, onEdit }: Props = $props();

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
</script>

<div class="group relative" id={"message-" + message.id}>
  <!-- 消息容器 -->
  <div class="flex justify-end">
    <!-- 消息内容 -->
    <div class="flex-1 min-w-0 text-right">
      <!-- 消息气泡 -->
      <div
        class="inline-block max-w-full px-4 py-3 rounded-2xl bg-base-200 text-base-content"
      >
        <!-- 消息内容 -->
        <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
          {message.content}
        </div>
      </div>

      <!-- 操作按钮 (hover显示) -->
      <div
        class="mt-2 gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200 flex justify-end"
      >
        <!-- 复制按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors"
          title="复制消息"
          onclick={handleCopy}
        >
          <Copy class="w-3.5 h-3.5" />
        </button>
        <!-- 编辑按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="编辑并重发"
          disabled={isOperating}
          onclick={handleEdit}
        >
          <Pencil class="w-3.5 h-3.5" />
        </button>
        <!-- 重发按钮 -->
        <button
          class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-200 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          title="重发消息"
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
