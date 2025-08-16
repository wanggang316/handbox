<script lang="ts">
  interface MessageAction {
    icon: string;
    tooltip: string;
  }

  interface Message {
    content: string;
    actions: MessageAction[];
  }

  interface Props {
    message: Message;
  }
  
  let { message }: Props = $props();
</script>

<!-- 消息内容 -->
<div class="flex-1 overflow-y-auto">
  <div class="w-full max-w-[800px] mx-auto p-4">
    <div class="text-[16px] leading-[1.73] text-black mb-4 whitespace-pre-line">
      {message.content}
    </div>
    <div class="flex gap-2">
      {#each message.actions as action}
        <button class="w-7 h-7 border border-gray-200 rounded flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" title={action.tooltip} aria-label={action.tooltip}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect width="14" height="14" x="8" y="8" rx="2" ry="2"/>
            <path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>
          </svg>
        </button>
      {/each}
    </div>
  </div>
</div>
