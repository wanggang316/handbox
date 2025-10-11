<script lang="ts">
  import ChatHeaderView from '$lib/components/chat/ChatHeader.svelte';
  import ChatContentView from '$lib/components/chat/ChatContent.svelte';
  import ChatInputView from '$lib/components/chat/ChatInput.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { uiState } from '$lib/states/ui.svelte';
  import { chatState, chatActions, hasActiveChat, currentChatModel } from '$lib/states/chat.svelte';
  import * as chatApi from '$lib/api/chat';
  import { goto } from '$app/navigation';
    import { messageStore } from '$lib/states';

  let chatId = $state('');
  let messageInput = $state('');
  let editingMessageId = $state<string | null>(null);

  // 从 URL 参数获取聊天 ID
  onMount(async () => {
    // 确保 chatState 已经初始化
    if (!chatState.isInitialized && !chatState.isInitializing) {
      await chatActions.initialize();
    }

    const urlParams = $page.url.searchParams;
    const newChatId = urlParams.get('id') || '';

    // 如果有 chatId，切换到对应聊天
    if (newChatId && newChatId !== chatId) {
      chatId = newChatId;
      try {
        await chatActions.switchToChat(chatId);
      } catch (error: any) {
        console.error('Failed to switch to chat:', error);
      }
    } else if (!newChatId) {
      // 清空当前聊天状态，显示默认界面
      chatId = '';
      chatState.currentChat = null;
    }
  });

  // 监听 URL 变化
  $effect(() => {
    const urlParams = $page.url.searchParams;
    const newChatId = urlParams.get('id') || '';

    if (newChatId !== chatId) {
      chatId = newChatId;
      console.log('Chat ID changed to:', chatId);

      if (chatId) {
        chatActions.switchToChat(chatId).catch(error => {
          console.error('Failed to switch to chat:', error);
        });
      } else {
        // 清空当前聊天状态，显示默认界面
        chatState.currentChat = null;
      }
    }
  });

  // 派生状态：当前聊天信息
  let currentChat = $derived(chatState.currentChat);

  // 派生状态：聊天标题和 ID
  let chatTitle = $derived(
    currentChat ? currentChat.name : 'HandBox'
  );
  let displayChatId = $derived(
    currentChat ? currentChat.id : ''
  );

  // 当前聊天的模型信息已通过导入的 currentChatModel 提供

  // 处理消息编辑
  function handleEditMessage(messageId: string, content: string) {
    console.log('handleEditMessage:', { messageId, content });
    editingMessageId = messageId;
    messageInput = content;
  }

  // 取消编辑
  function handleCancelEdit() {
    console.log('handleCancelEdit');
    editingMessageId = null;
    messageInput = '';
  }

  // 处理消息发送
  async function handleSendMessage(message: string) {
    console.log('handleSendMessage:', { message, editingMessageId });
    try {
      // 如果是编辑模式，调用 resendMessage
      if (editingMessageId) {
        if (!chatId) {
          throw new Error('未选择聊天会话');
        }
        await messageStore.resendMessage(chatId, editingMessageId, message);
        // 清除编辑状态
        editingMessageId = null;
        return;
      }

      // 否则是新消息
      if (!hasActiveChat()) {
        console.log('No active chat, creating new chat');
        // 如果没有活跃聊天，创建新聊天
        await chatActions.createChat("新会话");
        // 立即更新 URL，通知页面切换到新会话
        if (chatState.currentChat?.id) {
          await goto(`/chat?id=${chatState.currentChat.id}`);

          // 异步生成标题，不阻塞消息发送
          const chatId = chatState.currentChat.id;
          setTimeout(async () => {
            try {
              const result = await chatApi.generateChatTitle(chatId);
              if (result.title) {
                await chatActions.renameChat(chatId, result.title);
              }
            } catch (error) {
              console.error('Failed to generate title:', error);
            }
          }, 100); // 给一点延迟确保消息先发送
        }
      }

      // 使用简化的 messageStore 发送消息
      await messageStore.sendMessage(message, []);

    } catch (error) {
      console.error('Failed to send message:', error);
      // 如果是模型选择错误，可以在这里显示提示
      if (error instanceof Error && error.message.includes('选择模型')) {
        // TODO: 显示模型选择提示或自动打开模型选择弹框
        console.log('Model selection required');
      }
    }
  }
</script>

<!-- 聊天页面（将被 (app) 分组布局包裹） -->
<div class="flex-1 flex flex-col h-full">
  <!-- 固定在顶部的聊天头部 -->
  <div class="flex-shrink-0">
    <ChatHeaderView 
      chatId={displayChatId} 
      title={chatTitle}
      sidebarOpen={uiState.sidebarOpen}
    />
  </div>
  
  <!-- 可滚动的聊天内容区域，占据剩余空间 -->
  <div class="flex-1 min-h-0">
    <ChatContentView onEditMessage={handleEditMessage} />
  </div>

  <!-- 固定在底部的输入区域 -->
  <div class="flex-shrink-0 px-4 pb-4">
    <ChatInputView
      bind:messageInput={messageInput}
      bind:editingMessageId={editingMessageId}
      onSendMessage={handleSendMessage}
      onCancelEdit={handleCancelEdit}
    />
  </div>
</div>
