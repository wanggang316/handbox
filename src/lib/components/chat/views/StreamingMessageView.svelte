<script lang="ts">
  import { Bot } from 'lucide-svelte';

  interface Props {
    content: string;
    showCursor?: boolean;
  }

  let { 
    content,
    showCursor = true
  }: Props = $props();
</script>

<div class="group relative">
  <div class="flex gap-4">
    <!-- 助手头像 -->
    <div class="flex-shrink-0">
      <div class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center">
        <Bot class="w-4 h-4 text-blue-600" />
      </div>
    </div>

    <!-- 流式内容 -->
    <div class="flex-1 min-w-0">
      <div class="inline-block max-w-full p-4 rounded-2xl bg-gray-100 text-gray-900 shadow-sm">
        <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
          {content}
          <!-- 打字光标 -->
          {#if showCursor}
            <span class="animate-pulse">▋</span>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>