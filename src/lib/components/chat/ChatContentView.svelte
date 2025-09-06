<script lang="ts">
  import { chatState } from '$lib/states/chat.svelte';
  import { messageStore } from '$lib/states/message.svelte';
  import { Copy, RotateCcw, Trash2, Edit, User, Bot, Settings } from 'lucide-svelte';

  // 格式化时间戳
  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleTimeString('zh-CN', {
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  // 格式化token使用 (暂时不使用，直接在模板中格式化)
  // function formatTokens(message: Message): string {
  //   if (!message.inputTokens && !message.outputTokens) return '';
  //   
  //   const parts = [];
  //   if (message.inputTokens) parts.push(`输入: ${message.inputTokens}`);
  //   if (message.outputTokens) parts.push(`输出: ${message.outputTokens}`);
  //   if (message.totalTokens) parts.push(`总计: ${message.totalTokens}`);
  //   
  //   return parts.join(' | ');
  // }

  // 格式化持续时间
  function formatDuration(duration?: number): string {
    if (!duration) return '';
    
    if (duration < 1000) {
      return `${duration}ms`;
    } else {
      return `${(duration / 1000).toFixed(1)}s`;
    }
  }

  // 复制消息内容
  async function copyMessage(content: string) {
    try {
      await navigator.clipboard.writeText(content);
      console.log('Message copied successfully');
      // TODO: 集成 toast 提示系统显示成功提示
    } catch (error) {
      console.error('Failed to copy message:', error);
      // Fallback: 使用传统方法复制
      const textArea = document.createElement('textarea');
      textArea.value = content;
      document.body.appendChild(textArea);
      textArea.select();
      try {
        document.execCommand('copy');
        console.log('Message copied using fallback method');
      } catch (fallbackError) {
        console.error('Fallback copy also failed:', fallbackError);
      }
      document.body.removeChild(textArea);
    }
  }

  // 操作状态
  let operatingMessageId = $state<string | null>(null);

  // 重新生成消息
  async function regenerateMessage(messageId: string) {
    if (operatingMessageId) return; // 防止重复操作
    
    try {
      operatingMessageId = messageId;
      await messageStore.regenerateMessage(messageId);
      console.log('Message regenerated successfully');
    } catch (error) {
      console.error('Failed to regenerate message:', error);
      // TODO: 显示错误提示
    } finally {
      operatingMessageId = null;
    }
  }

  // 删除消息
  async function deleteMessage(messageId: string) {
    if (operatingMessageId || !chatState.currentChat) return; // 防止重复操作
    
    // 确认删除
    if (!confirm('确定要删除这条消息吗？')) {
      return;
    }

    try {
      operatingMessageId = messageId;
      await messageStore.removeMessage(chatState.currentChat.id, messageId);
      console.log('Message deleted successfully');
    } catch (error) {
      console.error('Failed to delete message:', error);
      // TODO: 显示错误提示
    } finally {
      operatingMessageId = null;
    }
  }

  // 编辑消息
  function editMessage(messageId: string) {
    // TODO: 实现消息编辑功能
    console.log('Editing message:', messageId);
  }

  // 获取当前聊天的消息 - 使用 Svelte 5 派生状态
  let messages = $derived(chatState.currentMessages);
  let isLoading = $derived(messageStore.isLoading);
  let isSending = $derived(messageStore.isSending);
  let streamingContent = $derived(messageStore.streamingContent);
  let streamingMessageId = $derived(messageStore.streamingMessageId);
</script>

<div class="flex flex-col flex-1 overflow-hidden">
  <!-- 消息列表 -->
  <div class="flex-1 overflow-y-auto">
    {#if isLoading && messages.length === 0}
      <!-- 加载状态 -->
      <div class="flex items-center justify-center h-full">
        <div class="flex items-center gap-2 text-gray-500">
          <div class="animate-spin w-4 h-4 border-2 border-current border-t-transparent rounded-full"></div>
          加载消息中...
        </div>
      </div>
    {:else if messages.length === 0}
      <!-- 空状态 -->
      <div class="flex items-center justify-center h-full">
        <div class="text-center text-gray-500">
          <Bot class="w-12 h-12 mx-auto mb-4 opacity-50" />
          <p class="text-lg mb-2">开始新的对话</p>
          <p class="text-sm">发送消息开始与 AI 助手交流</p>
        </div>
      </div>
    {:else}
      <!-- 消息列表 -->
      <div class="w-full max-w-4xl mx-auto p-4 space-y-6">
        {#each messages as message (message.id)}
          <div class="group relative">
            <!-- 消息容器 -->
            <div class="flex gap-4 {message.role === 'user' ? 'flex-row-reverse' : ''}">
              <!-- 头像 -->
              <div class="flex-shrink-0">
                <div class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center">
                  {#if message.role === 'user'}
                    <User class="w-4 h-4 text-gray-600" />
                  {:else if message.role === 'assistant'}
                    <Bot class="w-4 h-4 text-blue-600" />
                  {:else}
                    <Settings class="w-4 h-4 text-purple-600" />
                  {/if}
                </div>
              </div>

              <!-- 消息内容 -->
              <div class="flex-1 min-w-0 {message.role === 'user' ? 'text-right' : ''}">
                <!-- 消息气泡 -->
                <div class="inline-block max-w-full p-4 rounded-2xl {
                  message.role === 'user' 
                    ? 'bg-blue-600 text-white' 
                    : 'bg-gray-100 text-gray-900'
                } shadow-sm">
                  <!-- 消息内容 -->
                  <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
                    {message.content}
                  </div>

                  <!-- 模型和性能信息 -->
                  {#if message.role === 'assistant' && (message.modelId || message.inputTokens || message.duration)}
                    <div class="mt-3 pt-3 border-t border-gray-200 text-xs text-gray-500 space-y-1">
                      {#if message.modelId}
                        <div class="flex items-center gap-1">
                          <span class="font-medium">模型:</span>
                          <span>{message.modelId}</span>
                          {#if message.providerId}
                            <span class="text-gray-400">({message.providerId})</span>
                          {/if}
                        </div>
                      {/if}
                      
                      {#if message.inputTokens || message.outputTokens || message.totalTokens}
                        <div class="flex items-center gap-1">
                          <span class="font-medium">Token:</span>
                          <span>
                            {#if message.inputTokens}输入: {message.inputTokens}{/if}
                            {#if message.outputTokens} | 输出: {message.outputTokens}{/if}
                            {#if message.totalTokens} | 总计: {message.totalTokens}{/if}
                          </span>
                        </div>
                      {/if}
                      
                      {#if message.duration}
                        <div class="flex items-center gap-1">
                          <span class="font-medium">耗时:</span>
                          <span>{formatDuration(message.duration)}</span>
                        </div>
                      {/if}
                    </div>
                  {/if}
                </div>

                <!-- 时间戳 -->
                <div class="mt-2 text-xs text-gray-400 {message.role === 'user' ? 'text-right' : ''}">
                  {formatTime(message.createdAt)}
                </div>

                <!-- 消息操作按钮 -->
                <div class="mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-200 {
                  message.role === 'user' ? 'text-right' : ''
                }">
                  <div class="inline-flex gap-1">
                    <!-- 复制按钮 -->
                    <button
                      class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
                      title="复制消息"
                      onclick={() => copyMessage(message.content)}
                    >
                      <Copy class="w-3.5 h-3.5" />
                    </button>

                    {#if message.role === 'assistant'}
                      <!-- 重新生成按钮 -->
                      <button
                        class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                        title="重新生成"
                        disabled={operatingMessageId === message.id}
                        onclick={() => regenerateMessage(message.id)}
                      >
                        {#if operatingMessageId === message.id}
                          <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
                        {:else}
                          <RotateCcw class="w-3.5 h-3.5" />
                        {/if}
                      </button>
                    {/if}

                    {#if message.role === 'user'}
                      <!-- 编辑按钮 -->
                      <button
                        class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
                        title="编辑消息"
                        onclick={() => editMessage(message.id)}
                      >
                        <Edit class="w-3.5 h-3.5" />
                      </button>
                    {/if}

                    <!-- 删除按钮 -->
                    <button
                      class="p-1.5 text-gray-400 hover:text-red-600 hover:bg-red-50 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                      title="删除消息"
                      disabled={operatingMessageId === message.id}
                      onclick={() => deleteMessage(message.id)}
                    >
                      {#if operatingMessageId === message.id}
                        <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
                      {:else}
                        <Trash2 class="w-3.5 h-3.5" />
                      {/if}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        {/each}

        <!-- 流式响应中的消息 -->
        {#if streamingMessageId && streamingContent}
          <div class="group relative">
            <div class="flex gap-4">
              <!-- 助手头像 -->
              <div class="flex-shrink-0">
                <div class="w-8 h-8 rounded-full bg-gray-100 flex items-center justify-center">
                  <Bot class="w-4 h-4 text-blue-600" />
                </div>
              </div>

              <!-- 流式内容 -->
              <div class="flex-1 min-w-0">
                <div class="inline-block max-w-full p-4 rounded-2xl bg-gray-100 text-gray-900 shadow-sm">
                  <div class="whitespace-pre-wrap break-words text-[15px] leading-[1.6]">
                    {streamingContent}
                    <!-- 打字光标 -->
                    <span class="animate-pulse">▋</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  /* 自定义滚动条 */
  .overflow-y-auto::-webkit-scrollbar {
    width: 6px;
  }
  
  .overflow-y-auto::-webkit-scrollbar-track {
    background: transparent;
  }
  
  .overflow-y-auto::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.1);
    border-radius: 3px;
  }
  
  .overflow-y-auto::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.2);
  }
</style>