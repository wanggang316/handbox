<script lang="ts">
  import { messageStore } from "$lib/states/message.svelte";
  import { marked } from "marked";

  interface Props {
    content: string;
    reasoning?: string;
    showCursor?: boolean;
    providerId?: string;
  }

  let { content, reasoning, showCursor = true, providerId }: Props = $props();

  // 渲染 markdown 内容
  function renderMarkdown(content: string): string {
    const result = marked(content);
    return typeof result === 'string' ? result : '';
  }

  // 获取provider图标
  const providerIcon = $derived(() => {
    if (providerId) {
      console.log("providerId >>> :", providerId);
      console.log("messageStore.getProviderIcon(providerId) >>> :", messageStore.getProviderIcon(providerId));
      return messageStore.getProviderIcon(providerId);
    }
    return undefined;
  });
</script>

<div class="group relative">
  <div class="flex gap-4">
    <!-- 助手头像 -->
    <div class="flex-shrink-0">
      <div
        class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center"
      >
        <img src={providerIcon()} alt="" class="w-4 h-4 object-contain" />
      </div>
    </div>

    <!-- 流式内容 -->
    <div class="flex-1 min-w-0">
      <div
        class="inline-block max-w-full p-4 rounded-2xl bg-gray-100 text-gray-900 shadow-sm"
      >
        <!-- 推理过程（如果有） -->
        {#if reasoning}
          <div class="mb-4 p-3 bg-blue-50 border border-blue-200 rounded-lg">
            <div class="flex items-center gap-2 mb-2">
              <div class="w-2 h-2 bg-blue-500 rounded-full"></div>
              <span class="text-sm font-medium text-blue-700">推理过程</span>
            </div>
            <div class="text-sm text-blue-800 break-words leading-relaxed reasoning-content">
              {@html renderMarkdown(reasoning)}
              <!-- 推理过程的光标 -->
              {#if showCursor}
                <span class="animate-pulse">▋</span>
              {/if}
            </div>
          </div>
        {/if}

        <!-- 消息内容 -->
        <div class="break-words text-[15px] leading-[1.6] markdown-content">
          {@html renderMarkdown(content)}
          <!-- 打字光标 -->
          {#if showCursor}
            <span class="animate-pulse">▋</span>
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>
