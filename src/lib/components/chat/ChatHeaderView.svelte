<script lang="ts">
    import { Ellipsis } from "@lucide/svelte";
    import IconButton from "../ui/IconButton.svelte";
    import ChatSettings from "./settings/ChatSettings.svelte";
    import { uiState } from "$lib/states/ui.svelte";
    import Button from "../ui/Button.svelte";
    import { RefreshCw } from "@lucide/svelte";

  interface Props {
    chatId?: string;
    title?: string;
    sidebarOpen?: boolean;
  }
  
  let { 
    chatId = '', 
    title = 'AI 助手',
    sidebarOpen = true 
  }: Props = $props();

  const CHAT_SETTINGS_MODAL = 'chat-settings';

  function handleChatSettings() {
    uiState.openModal(CHAT_SETTINGS_MODAL);
  }

  function handleCloseChatSettings() {
    uiState.closeModal(CHAT_SETTINGS_MODAL);
  }

  function handleRefresh() {
    console.log('刷新状态');
  }
</script>

<!-- 顶部栏 -->
<header class="h-[50px] px-4 border-b border-gray-200 flex items-center justify-between">
  <div class="transition-all duration-300" class:ml-[120px]={!sidebarOpen}>
    <h1 class="text-base font-medium text-gray-900">
      {title}
      {#if chatId}
        <span class="text-xs text-gray-500 ml-2">聊天 ID: {chatId}</span>
      {/if}
    </h1>
    
  </div>
  <div class="flex items-center gap-2 relative z-[10001]">
    {#if chatId}
      <IconButton 
        icon={Ellipsis} 
        ariaLabel="设置" 
        on:click={handleChatSettings} 
      />
    {/if}
</header>

<!-- 聊天设置模态框 -->
<ChatSettings 
  open={uiState.modals[CHAT_SETTINGS_MODAL] || false}
  onClose={handleCloseChatSettings}
/>
