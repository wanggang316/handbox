<script lang="ts">
  import Modal from '../../ui/Modal.svelte';
  import ChatSettingSidebar from './SettingsSidebar.svelte';
  import PromptSettings from './SettingsPrompt.svelte';
  import ModelSettings from './SettingsModel.svelte';
  import McpSettings from './SettingsMcp.svelte';
  import { chatState } from '$lib/states/chat.svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  // 当前活跃的选项卡
  let activeTab = $state('prompt');

  // 记录上一个聊天 ID，用于检测切换
  let lastChatId = $state(chatState.currentChat?.id);

  // 监听聊天 ID 变化，重置 activeTab
  $effect(() => {
    if (chatState.currentChat?.id !== lastChatId) {
      activeTab = 'prompt'; // 重置为默认标签
      lastChatId = chatState.currentChat?.id;
    }
  });

  function handleTabChange(tab: string) {
    console.log('切换到:', tab);
    activeTab = tab;
  }

</script>

<Modal {open} title="" {onClose}>
  <div class="flex w-[650px] h-[600px]">
    <!-- 左侧边栏 -->
    <ChatSettingSidebar
      {activeTab}
      onTabChange={handleTabChange}
    />

    <!-- 右侧内容区域 -->
    <div class="flex-1 flex flex-col my-6 ml-2 mr-4">
      <!-- 内容区域 -->
      <div class="flex-1 overflow-y-auto">
        {#if activeTab === 'prompt'}
          <PromptSettings />
        {:else if activeTab === 'model'}
          <ModelSettings />
        {:else if activeTab === 'mcp'}
          <McpSettings />
        {/if}
      </div>

    </div>
  </div>
</Modal>