<script lang="ts">
  import type { Message } from '$lib/types';

  interface Props {
    message: Message;
  }

  let { 
    message 
  }: Props = $props();

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  </script>

<div class="group relative">
  <!-- 消息容器 -->
  <div class="flex justify-end">
    <!-- 消息内容 -->
    <div class="flex-1 min-w-0 text-right">
      <!-- 消息气泡 -->
      <div class="inline-block max-w-full px-4 py-3 rounded-2xl bg-base-200 text-base-content">
        <!-- 消息内容 -->
        <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
          {message.content}
        </div>
      </div>

      <!-- 时间戳 -->
      <div class="mt-2 text-xs text-base-content/60 text-right">
        {formatTime(message.createdAt)}
      </div>
    </div>
  </div>
</div>
