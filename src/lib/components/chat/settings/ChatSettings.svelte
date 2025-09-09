<script lang="ts">
  import Modal from '../../ui/Modal.svelte';
  import ChatSettingSidebar from './ChatSettingSidebar.svelte';
  import PromptSettings from './PromptSettings.svelte';
  import ModelSettings from './ModelSettings.svelte';
  import McpSettings from './McpSettings.svelte';
  import Button from '../../ui/Button.svelte';
  import { Download, Upload, Save } from '@lucide/svelte';

  interface Props {
    open: boolean;
    onClose: () => void;
  }

  let { open, onClose }: Props = $props();

  // 当前活跃的选项卡
  let activeTab = $state('prompt');

  // 聊天设置状态
  let systemPrompt = $state('');
  let temperature = $state('0.7');
  let topP = $state('0.9');
  let streamResponse = $state(true);
  let maxTokens = $state('0');
  let contextLength = $state('10');
  let enabledMcpServers = $state<string[]>([]);

  function handleTabChange(tab: string) {
    console.log('切换到:', tab);
    activeTab = tab;
  }

  function handlePromptSave(prompt: string) {
    systemPrompt = prompt;
    console.log('保存提示词:', prompt);
  }

  function handleModelSave(settings: any) {
    temperature = settings.temperature;
    topP = settings.topP;
    streamResponse = settings.streamResponse;
    maxTokens = settings.maxTokens;
    contextLength = settings.contextLength;
    console.log('保存模型参数:', settings);
  }

  function handleMcpSave(serverIds: string[]) {
    enabledMcpServers = serverIds;
    console.log('保存 MCP 设置:', serverIds);
  }

  function handleSaveAll() {
    const allSettings = {
      systemPrompt,
      temperature,
      topP,
      streamResponse,
      maxTokens,
      contextLength,
      enabledMcpServers
    };
    console.log('保存所有设置:', allSettings);
    // 这里可以添加保存设置的逻辑
    onClose();
  }

  function handleExportSettings() {
    const settings = {
      systemPrompt,
      temperature,
      topP,
      streamResponse,
      maxTokens,
      contextLength,
      enabledMcpServers
    };
    
    const blob = new Blob([JSON.stringify(settings, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'chat-settings.json';
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleImportSettings() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const reader = new FileReader();
        reader.onload = (e) => {
          try {
            const settings = JSON.parse(e.target?.result as string);
            systemPrompt = settings.systemPrompt ?? '';
            temperature = settings.temperature ?? '0.7';
            topP = settings.topP ?? '0.9';
            streamResponse = settings.streamResponse ?? true;
            maxTokens = settings.maxTokens ?? '2048';
            contextLength = settings.contextLength ?? '4096';
            enabledMcpServers = settings.enabledMcpServers ?? [];
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
          <PromptSettings 
            systemPrompt={systemPrompt}
            onSave={handlePromptSave}
          />
        {:else if activeTab === 'model'}
          <ModelSettings 
            {temperature}
            {topP}
            {streamResponse}
            {maxTokens}
            {contextLength}
            onSave={handleModelSave}
          />
        {:else if activeTab === 'mcp'}
          <McpSettings 
            onSave={handleMcpSave}
          />
        {/if}
      </div>
      
    </div>
  </div>
</Modal>


