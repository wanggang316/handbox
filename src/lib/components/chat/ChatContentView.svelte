<script lang="ts">
  import { chatState } from '$lib/states/chat.svelte';
  import { messageStore } from '$lib/states/message.svelte';
  import { Bot } from 'lucide-svelte';
  import type { Message } from '$lib/types';

  // Import child components
  import UserMessageView from './views/UserMessageView.svelte';
  import AssistantMessageView from './views/AssistantMessageView.svelte';
  import ConfirmModal from '$lib/components/ui/ConfirmModal.svelte';

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

  // 确认对话框状态
  let showDeleteConfirm = $state(false);
  let showResendConfirm = $state(false);
  let pendingMessageId = $state<string | null>(null);

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

  // 删除消息 - 显示确认对话框
  function deleteMessage(messageId: string) {
    if (operatingMessageId || !chatState.currentChat?.id) return; // 防止重复操作
    pendingMessageId = messageId;
    showDeleteConfirm = true;
  }

  // 确认删除消息
  async function confirmDeleteMessage() {
    if (!pendingMessageId || !chatState.currentChat?.id) return;

    try {
      operatingMessageId = pendingMessageId;
      await messageStore.removeMessage(chatState.currentChat.id, pendingMessageId);
      console.log('Message deleted successfully');
    } catch (error) {
      console.error('Failed to delete message:', error);
      // TODO: 显示错误提示
    } finally {
      operatingMessageId = null;
      pendingMessageId = null;
    }
  }

  // 取消删除
  function cancelDelete() {
    pendingMessageId = null;
  }

  // 编辑消息
  function editMessage(messageId: string) {
    // TODO: 实现消息编辑功能
    console.log('Editing message:', messageId);
  }

  // 重发用户消息 - 显示确认对话框
  function resendMessage(messageId: string) {
    if (operatingMessageId) return; // 防止重复操作
    pendingMessageId = messageId;
    showResendConfirm = true;
  }

  // 确认重发消息
  async function confirmResendMessage() {
    if (!pendingMessageId) return;

    try {
      operatingMessageId = pendingMessageId;
      await messageStore.resendMessage(pendingMessageId);
      console.log('Message resent successfully');
    } catch (error) {
      console.error('Failed to resend message:', error);
      // TODO: 显示错误提示
    } finally {
      operatingMessageId = null;
      pendingMessageId = null;
    }
  }

  // 取消重发
  function cancelResend() {
    pendingMessageId = null;
  }

  // 当前聊天ID的派生状态
  let currentChatId = $derived(chatState.currentChat?.id);

  // 派生状态：获取当前聊天的消息 - 使用响应式getter
  let messages = $derived(currentChatId ? messageStore.getMessagesReactive(currentChatId) : []);
  let isLoading = $derived(messageStore.isLoading);
  let streamingContent = $derived(messageStore.streamingContent);
  let streamingReasoning = $derived(messageStore.streamingReasoning);
  let streamingToolCalls = $derived(messageStore.streamingToolCalls);
  let streamingMessageId = $derived(messageStore.streamingMessageId);
  let isReasoning = $derived(messageStore.isReasoning);
  let isMessageLoading = $derived(messageStore.isMessageLoading);

  let streamingMessage = $derived(
    {
          id: streamingMessageId,
          chatId: currentChatId ?? '',
          role: 'assistant' as const,
          content: streamingContent ?? '',
          reasoning: streamingReasoning,
          toolCalls: streamingToolCalls ? streamingToolCalls.map((call, index) => ({
            index: call.index || index,
            id: call.id,
            toolType: call.toolType,
            function: call.function
          })) : undefined,
          createdAt: Date.now(),
          config: {
            modelId: chatState.currentChat?.modelId,
            providerId: chatState.currentChat?.providerId,
            temperature: chatState.currentChat?.temperature,
            topP: chatState.currentChat?.topP,
            maxTokens: chatState.currentChat?.maxTokens,
            stream: chatState.currentChat?.stream,
            systemPrompt: chatState.currentChat?.systemPrompt,
            mcpServers: chatState.currentChat?.mcpServers,
          },
          updatedAt: Date.now(),
        } as Message
  );

  // 监听聊天切换，自动加载消息（使用单独的 effect 避免循环）
  let lastLoadedChatId = $state<string | null>(null);

  $effect(() => {
    if (currentChatId && currentChatId !== lastLoadedChatId) {
      // 检查是否已经有消息，没有则加载
      const existingMessages = messageStore.getMessages(currentChatId);
      // 如果正在发送消息或有流式响应，不要加载（避免覆盖本地消息）
      if (existingMessages.length === 0 && !messageStore.isSending && !messageStore.streamingMessageId) {
        messageStore.loadMessages(currentChatId).catch(error => {
          console.error('Failed to load messages:', error);
        }).finally(() => {
          lastLoadedChatId = currentChatId;
        });
      } else {
        lastLoadedChatId = currentChatId;
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
    if (streamingContent || streamingToolCalls) {
      setTimeout(scrollToBottom, 50);
    }
  });
</script>

<div class="flex flex-col h-full">
  <!-- 消息列表 -->
  <div bind:this={messagesContainer} class="flex-1 overflow-y-auto">
    {#if isLoading && messages.length === 0 && !streamingMessageId}
      <!-- 加载状态 -->
      <div class="flex items-center justify-center h-full">
        <div class="flex items-center gap-2 text-base-content/70">
          <div class="animate-spin w-4 h-4 border-2 border-current border-t-transparent rounded-full"></div>
          加载消息中...
        </div>
      </div>
    {:else if messages.length === 0 && !streamingMessageId}
      <!-- 空状态 -->
      <div class="flex items-center justify-center h-full">
        <div class="text-center text-base-content/70">
          <Bot class="w-12 h-12 mx-auto mb-4" />
          <p class="text-lg mb-2">开始新的对话</p>
        </div>
      </div>
    {:else}
      <!-- 消息列表 -->
      <div class="w-full mx-auto max-w-[800px] py-4 px-1 space-y-6">
        {#each messages as message (message.id)}
          {#if message.role === 'user'}
            <UserMessageView
              {message}
              isOperating={operatingMessageId === message.id}
              onResend={resendMessage}
              onCopy={copyMessage}
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
                <div class="inline-block max-w-full p-2 px-4 rounded-full bg-accent/10 text-accent text-sm">
                  {message.content}
                </div>
              </div>
            </div>
          {/if}
        {/each}

        <!-- 消息加载状态或流式响应中的消息 -->
        {#if isMessageLoading || (streamingMessageId && (streamingContent || streamingReasoning || streamingToolCalls))}
          <AssistantMessageView
            message={streamingMessage ?? undefined}
            isStreaming={!!streamingMessage}
            isReasoning={!!isReasoning}
            isMessageLoading={isMessageLoading}
          />
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- 删除消息确认对话框 -->
<ConfirmModal
  open={showDeleteConfirm}
  title="确认删除"
  message="确定要删除这条消息吗？"
  confirmText="删除"
  cancelText="取消"
  confirmButtonStyle="danger"
  onConfirm={confirmDeleteMessage}
  onCancel={cancelDelete}
  onClose={() => showDeleteConfirm = false}
/>

<!-- 重发消息确认对话框 -->
<ConfirmModal
  open={showResendConfirm}
  title="确认重发"
  message="重发此消息将删除它之后的所有消息，确定要继续吗？"
  confirmText="重发"
  cancelText="取消"
  confirmButtonStyle="accent"
  onConfirm={confirmResendMessage}
  onCancel={cancelResend}
  onClose={() => showResendConfirm = false}
/>

<style>
  /* 自定义滚动条 */
  .overflow-y-auto::-webkit-scrollbar {
    width: 6px;
  }
  
  .overflow-y-auto::-webkit-scrollbar-track {
    background: transparent;
  }
  
  .overflow-y-auto::-webkit-scrollbar-thumb {
    background: color-mix(in oklch, var(--base-content) 15%, transparent);
    border-radius: 3px;
  }
  
  .overflow-y-auto::-webkit-scrollbar-thumb:hover {
    background: color-mix(in oklch, var(--base-content) 25%, transparent);
  }
</style>
