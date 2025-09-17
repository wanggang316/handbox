<script lang="ts">
  import Modal from '../../ui/Modal.svelte';
  import ChatSettingSidebar from './ChatSettingSidebar.svelte';
  import PromptSettings from './PromptSettings.svelte';
  import ModelSettings from './ModelSettings.svelte';
  import McpSettings from './McpSettings.svelte';
  import { chatState } from '$lib/states/chat.svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  // 当前活跃的选项卡
  let activeTab = $state('prompt');

  function handleTabChange(tab: string) {
    console.log('切换到:', tab);
    activeTab = tab;
  }

  function handleExportSettings() {
    if (!chatState.currentChat) {
      console.warn('No current chat to export settings from');
      return;
    }

    const settings = {
      systemPrompt: chatState.currentChat.systemPrompt || '',
      temperature: chatState.currentChat.temperature || 0.7,
      topP: chatState.currentChat.topP || 1.0,
      streamResponse: chatState.currentChat.stream ?? true,
      maxTokens: chatState.currentChat.maxTokens || 4000,
      mcpServers: chatState.currentChat.mcpServers || []
    };

    const blob = new Blob([JSON.stringify(settings, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'chat-settings.json';
    a.click();
    URL.revokeObjectURL(url);
  }

  async function handleImportSettings() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = async (e) => {
          try {
            const settings = JSON.parse(e.target?.result as string);

            // Import settings using the chat actions
            const { chatActions } = await import('$lib/states/chat.svelte');

            await chatActions.updateChatSettings({
              systemPrompt: settings.systemPrompt,
              temperature: settings.temperature,
              topP: settings.topP,
              stream: settings.streamResponse ?? settings.stream,
              maxTokens: settings.maxTokens,
              mcpServers: settings.mcpServers
            });

            console.log('Settings imported successfully');
          } catch (error) {
            console.error('导入设置失败:', error);
          }
        };
        reader.readAsText(file);
      }
    };
    input.click();
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