<script lang="ts">
  interface Props {
    sessionId?: string;
    title?: string;
  }
  
  let { sessionId = '', title = 'HandBox - AI 助手' }: Props = $props();
</script>

<!-- 顶部栏 -->
<header class="p-4 border-b border-gray-200 bg-white flex items-center justify-between">
  <div>
    <h1 class="text-base font-medium text-gray-900">
      {title}
    </h1>
    {#if sessionId}
      <p class="text-xs text-gray-500 mt-1">会话 ID: {sessionId}</p>
    {/if}
  </div>
  <div class="flex gap-2">
    <button class="w-8 h-8 border border-gray-200 rounded-md flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" aria-label="打开面板">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M8 16H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2m-6 12h8a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2z"/>
      </svg>
    </button>
    <button class="w-8 h-8 border border-gray-200 rounded-md flex items-center justify-center text-gray-500 hover:bg-gray-50 transition-colors" aria-label="编辑">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M11 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-5m-1.414-9.414a2 2 0 1 1 2.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
      </svg>
    </button>
  </div>
</header>
