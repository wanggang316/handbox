<script lang="ts">
  import { messageStore } from "$lib/states/message.svelte";

  interface Props {
    content: string;
    showCursor?: boolean;
    providerId?: string;
  }

  let { content, showCursor = true, providerId }: Props = $props();

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
