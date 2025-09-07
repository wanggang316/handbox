<script lang="ts">
  import { Copy, Edit, Trash2, User } from 'lucide-svelte';
  import type { Message } from '$lib/types';

  interface Props {
    message: Message;
    isOperating?: boolean;
    onCopy?: (content: string) => void;
    onEdit?: (messageId: string) => void;
    onDelete?: (messageId: string) => void;
  }

  let { 
    message,
    isOperating = false,
    onCopy,
    onEdit,
    onDelete 
  }: Props = $props();

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  // 处理操作
  function handleCopy() {
    onCopy?.(message.content);
  }

  function handleEdit() {
    onEdit?.(message.id);
  }

  function handleDelete() {
    onDelete?.(message.id);
  }
</script>

<div class="group relative">
  <!-- 消息容器 -->
  <div class="flex gap-4 flex-row-reverse">
    <!-- 头像 -->
    <div class="flex-shrink-0">
      <div class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center">
        <User class="w-4 h-4 text-gray-600" />
      </div>
    </div>

    <!-- 消息内容 -->
    <div class="flex-1 min-w-0 text-right">
      <!-- 消息气泡 -->
      <div class="inline-block max-w-full p-4 rounded-2xl bg-blue-600 text-white shadow-sm">
        <!-- 消息内容 -->
        <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
          {message.content}
        </div>
      </div>

      <!-- 时间戳 -->
      <div class="mt-2 text-xs text-gray-400 text-right">
        {formatTime(message.createdAt)}
      </div>

      <!-- 消息操作按钮 -->
      <div class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200 text-right">
        <div class="inline-flex gap-1">
          <!-- 复制按钮 -->
          <button
            class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
            title="复制消息"
            onclick={handleCopy}
          >
            <Copy class="w-3.5 h-3.5" />
          </button>

          <!-- 编辑按钮 -->
          <button
            class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
            title="编辑消息"
            onclick={handleEdit}
          >
            <Edit class="w-3.5 h-3.5" />
          </button>

          <!-- 删除按钮 -->
          <button
            class="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="删除消息"
            disabled={isOperating}
            onclick={handleDelete}
          >
            {#if isOperating}
              <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
            {:else}
              <Trash2 class="w-3.5 h-3.5" />
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
</div>