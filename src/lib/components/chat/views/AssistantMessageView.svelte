<script lang="ts">
  import { Copy, RotateCcw, Trash2, Bot } from 'lucide-svelte';
  import type { Message } from '$lib/types';

  interface Props {
    message: Message;
    isOperating?: boolean;
    onCopy?: (content: string) => void;
    onRegenerate?: (messageId: string) => void;
    onDelete?: (messageId: string) => void;
  }

  let { 
    message,
    isOperating = false,
    onCopy,
    onRegenerate,
    onDelete 
  }: Props = $props();

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  // 格式化持续时间
  function formatDuration(duration?: number): string {
    if (!duration) return '';
    
    if (duration < 1000) {
      return `${duration}ms`;
    } else {
      return `${(duration / 1000).toFixed(1)}s`;
    }
  }

  // 处理操作
  function handleCopy() {
    onCopy?.(message.content);
  }

  function handleRegenerate() {
    onRegenerate?.(message.id);
  }

  function handleDelete() {
    onDelete?.(message.id);
  }
</script>

<div class="group relative">
  <!-- 消息容器 -->
  <div class="flex gap-4">
    <!-- 头像 -->
    <div class="flex-shrink-0">
      <div class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center">
        <Bot class="w-4 h-4 text-blue-600" />
      </div>
    </div>

    <!-- 消息内容 -->
    <div class="flex-1 min-w-0">
      <!-- 消息气泡 -->
      <div class="inline-block max-w-full p-4 rounded-2xl bg-gray-100 text-gray-900 shadow-sm">
        <!-- 消息内容 -->
        <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
          {message.content}
        </div>

        <!-- 模型和性能信息 -->
        {#if message.config?.modelId || message.inputTokens || message.duration}
          <div class="mt-3 pt-3 border-t border-gray-200 text-xs text-gray-500 space-y-1">
            {#if message.config?.modelId}
              <div class="flex items-center gap-1">
                <span class="font-medium">模型:</span>
                <span>{message.config.modelId}</span>
                {#if message.config.providerId}
                  <span class="text-gray-400">({message.config.providerId})</span>
                {/if}
              </div>
            {/if}
            
            {#if message.inputTokens || message.outputTokens || message.totalTokens}
              <div class="flex items-center gap-1">
                <span class="font-medium">Token:</span>
                <span>
                  {#if message.inputTokens}输入: {message.inputTokens}{/if}
                  {#if message.outputTokens} | 输出: {message.outputTokens}{/if}
                  {#if message.totalTokens} | 总计: {message.totalTokens}{/if}
                </span>
              </div>
            {/if}
            
            {#if message.duration}
              <div class="flex items-center gap-1">
                <span class="font-medium">耗时:</span>
                <span>{formatDuration(message.duration)}</span>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <!-- 时间戳 -->
      <div class="mt-2 text-xs text-gray-400">
        {formatTime(message.createdAt)}
      </div>

      <!-- 消息操作按钮 -->
      <div class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200">
        <div class="inline-flex gap-1">
          <!-- 复制按钮 -->
          <button
            class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
            title="复制消息"
            onclick={handleCopy}
          >
            <Copy class="w-3.5 h-3.5" />
          </button>

          <!-- 重新生成按钮 -->
          <button
            class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="重新生成"
            disabled={isOperating}
            onclick={handleRegenerate}
          >
            {#if isOperating}
              <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
            {:else}
              <RotateCcw class="w-3.5 h-3.5" />
            {/if}
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