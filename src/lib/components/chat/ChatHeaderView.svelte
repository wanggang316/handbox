<script lang="ts">
    import { Ellipsis } from "@lucide/svelte";
    import IconButton from "../ui/IconButton.svelte";

  interface Props {
    sessionId?: string;
    title?: string;
    sidebarOpen?: boolean;
  }
  
  let { 
    sessionId = '', 
    title = 'HandBox - AI 助手',
    sidebarOpen = true 
  }: Props = $props();

  function handleChatSettings() {
    console.log('设置按钮被点击');
    // 这里可以添加设置功能的逻辑
  }
</script>

<!-- 顶部栏 -->
<header class="h-[50px] px-4 border-b border-gray-200 flex items-center justify-between">
  <div class="transition-all duration-300" class:ml-[120px]={!sidebarOpen}>
    <h1 class="text-base font-medium text-gray-900">
      {title}
      {#if sessionId}
        <span class="text-xs text-gray-500 ml-2">会话 ID: {sessionId}</span>
      {/if}
    </h1>
    
  </div>
  <div class="flex items-center gap-2 relative z-[10001]">
    <IconButton 
      icon={Ellipsis} 
      ariaLabel="设置" 
      on:click={handleChatSettings} 
    />
  </div>
</header>
