<script lang="ts">
  import { chatState } from '$lib/states/chat.svelte';
  import { messageStore } from '$lib/states/message.svelte';
  import { Bot } from 'lucide-svelte';
  
  // Import child components
  import UserMessageView from './views/UserMessageView.svelte';
  import AssistantMessageView from './views/AssistantMessageView.svelte';
  import StreamingMessageView from './views/StreamingMessageView.svelte';

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
  let messages = $derived(messageStore.getCurrentMessages(chatState.currentChat?.id));
  let isLoading = $derived(messageStore.isLoading);
  let isSending = $derived(messageStore.isSending);
  let streamingContent = $derived(messageStore.streamingContent);
  let streamingReasoning = $derived(messageStore.streamingReasoning);
  let streamingMessageId = $derived(messageStore.streamingMessageId);

  // 监听聊天切换，自动加载消息
  $effect(() => {
    const currentChat = chatState.currentChat;
    if (currentChat?.id) {
      // 检查是否已经有消息，没有则加载
      const existingMessages = messageStore.getMessages(currentChat.id);
      if (existingMessages.length === 0) {
        messageStore.loadMessages(currentChat.id).catch(error => {
          console.error('Failed to load messages:', error);
        });
      }
    }
  });
  
  // 消息容器引用
  let messagesContainer: HTMLDivElement;
  
  // 自动滚动到底部
  function scrollToBottom() {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }
  
  // 监听消息变化，自动滚动到底部
  $effect(() => {
    if (messages.length > 0) {
      // 使用 setTimeout 确保 DOM 更新完成后再滚动
      setTimeout(scrollToBottom, 100);
    }
  });
  
  // 监听流式内容变化，自动滚动
  $effect(() => {
    if (streamingContent) {
      setTimeout(scrollToBottom, 50);
    }
  });
</script>

<div class="flex flex-col h-full">
  <!-- 消息列表 -->
  <div bind:this={messagesContainer} class="flex-1 overflow-y-auto bg-blue-100">
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
      <div class="w-full mx-auto max-w-[800px] py-4 px-1 space-y-6 bg-green-100">
        {#each messages as message (message.id)}
          {#if message.role === 'user'}
            <UserMessageView 
              {message}
            />
          {:else if message.role === 'assistant'}
            <AssistantMessageView 
              {message}
              isOperating={operatingMessageId === message.id}
              onCopy={copyMessage}
              onRegenerate={regenerateMessage}
              onDelete={deleteMessage}
            />
          {:else}
            <!-- System message fallback -->
            <div class="group relative">
              <div class="flex gap-4 justify-center">
                <div class="inline-block max-w-full p-2 px-4 rounded-full bg-purple-100 text-purple-800 text-sm">
                  {message.content}
                </div>
              </div>
            </div>
          {/if}
        {/each}

        <!-- 流式响应中的消息 -->
        {#if streamingMessageId && (streamingContent || streamingReasoning)}
          <StreamingMessageView 
            content={streamingContent}
            reasoning={streamingReasoning}
            showCursor={true}
          />
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