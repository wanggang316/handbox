<script lang="ts">
  let messageInput = $state('');
  let selectedModel = $state('DeepSeek R1');
  let showModelDropdown = $state(false);

  const models = ['DeepSeek R1', 'Claude 3.5 Sonnet', 'GPT-4', 'Gemini Pro'];

  const currentMessage = {
    content: `核心思路：\n\n• Day 1： 聚焦威海市区 - 国际海水浴场、历史与繁华\n• Day 2： 威海精华（那香海/鸡鸣岛/成山头 三选一） + 下午转场烟台 + 烟台山日落风光\n• Day 3： 烟台精华（蓬莱阁或市区深度/养马岛）`,
    actions: [
      { icon: 'copy', tooltip: 'Copy' },
      { icon: 'edit', tooltip: 'Edit' },
      { icon: 'refresh', tooltip: 'Refresh' },
      { icon: 'share', tooltip: 'Share' }
    ]
  };

  function toggleModelDropdown() {
    showModelDropdown = !showModelDropdown;
  }

  function selectModel(model: string) {
    selectedModel = model;
    showModelDropdown = false;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }

  function sendMessage() {
    if (!messageInput.trim()) return;
    console.log('Sending message:', messageInput);
    messageInput = '';
  }
</script>

<!-- 顶部栏 -->
<header class="p-4 border-b border-gray-200 bg-white flex items-center justify-between">
  <div>
    <h1 class="text-base font-medium text-gray-900">存细介绍如何使用 WebView 方式调试</h1>
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

<!-- 消息内容 -->
<div class="flex-1 overflow-y-auto">
  <div class="max-w-3xl mx-auto">
    <div class="text-[20px] leading-[1.73] text-black mb-4 whitespace-pre-line">
      {currentMessage.content}
    </div>
    <div class="flex gap-2">
      {#each currentMessage.actions as action}
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

<!-- 输入区域 -->
<div class="pt-4">
  <div class="relative">
    <textarea
      bind:value={messageInput}
      placeholder="在这里输入消息，按 Enter 发送"
      onkeydown={handleKeydown}
      rows="3"
      class="w-full bg-[#f7f7f7] rounded-[20px] border border-[#ebeaea] text-[20px] leading-[1.2] text-[#7e7e7f] px-12 py-4 pr-48 outline-none resize-none"
    ></textarea>
    <button class="absolute left-3 top-1/2 -translate-y-1/2 w-9 h-9 rounded-md border border-[#e5e5e5] bg-white text-[#1e1e1e] flex items-center justify-center" aria-label="添加">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#1e1e1e" stroke-width="2"><path d="M12 5v14m-7-7h14"/></svg>
    </button>
    <div class="absolute right-3 top-1/2 -translate-y-1/2 flex items-center gap-3">
      <div class="relative">
        <button onclick={toggleModelDropdown} class="h-8 px-3 rounded-md border border-[#e5e5e5] text-[20px] leading-[1.2] text-black flex items-center gap-1 bg-white" aria-haspopup="listbox" aria-expanded={showModelDropdown} aria-label="选择模型">
          {selectedModel}
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#757575" stroke-width="2"><polyline points="6,9 12,15 18,9"/></svg>
        </button>
        {#if showModelDropdown}
          <div class="absolute bottom-full right-0 mb-1 min-w-40 bg-white border border-[#e5e5e5] rounded-lg shadow-md z-10">
            {#each models as model}
              <button class="w-full px-3 py-2 text-left text-[16px] {model === selectedModel ? 'bg-blue-50 text-blue-600 font-medium' : 'text-black'}" onclick={() => selectModel(model)}>{model}</button>
            {/each}
          </div>
        {/if}
      </div>
      <button class="w-7 h-7 rounded border border-[#e5e5e5] flex items-center justify-center" aria-label="设置">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#1e1e1e" stroke-width="2"><circle cx="12" cy="12" r="3"/></svg>
      </button>
      <button class="w-9 h-9 rounded-full bg-[#1f1f1f] text-white flex items-center justify-center disabled:opacity-50" onclick={sendMessage} disabled={!messageInput.trim()} aria-label="发送">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2"><path d="M22 2L11 13"/><polygon points="22,2 15,22 11,13 2,9 22,2"/></svg>
      </button>
    </div>
  </div>
</div>


