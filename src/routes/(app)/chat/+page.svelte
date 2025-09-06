<script lang="ts">
  import ChatHeaderView from '$lib/components/chat/ChatHeaderView.svelte';
  import ChatContentView from '$lib/components/chat/ChatContentView.svelte';
  import ChatInputView from '$lib/components/chat/ChatInputView.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { uiState } from '$lib/states/ui.svelte';
  import { chatState } from '$lib/states/chat.svelte';

  let chatId = $state('');
  let messageInput = $state('');

  // 从 URL 参数获取聊天 ID
  onMount(async () => {
    const urlParams = $page.url.searchParams;
    const newChatId = urlParams.get('id') || '';
    console.log('Current chat ID:', newChatId);
    
    // 如果有 chatId，切换到对应聊天
    if (newChatId && newChatId !== chatId) {
      chatId = newChatId;
      try {
        await chatState.switchToChat(chatId);
      } catch (error) {
        console.error('Failed to switch to chat:', error);
      }
    } else if (!newChatId) {
      // 清空当前聊天状态，显示默认界面
      chatId = '';
      chatState.currentChat = null;
      chatState.messages = [];
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
        chatState.switchToChat(chatId).catch(error => {
          console.error('Failed to switch to chat:', error);
        });
      } else {
        // 清空当前聊天状态，显示默认界面
        chatState.currentChat = null;
        chatState.messages = [];
      }
    }
  });

  // 派生状态：当前聊天信息
  let currentChat = $derived(chatState.currentChat);

  // 派生状态：聊天标题和 ID
  let chatTitle = $derived(
    currentChat ? currentChat.name : 'HandBox - AI 助手'
  );
  let displayChatId = $derived(
    currentChat ? currentChat.id : ''
  );

  // 派生状态：当前聊天的模型信息（直接从 currentChat 获取）
  let currentChatModel = $derived(() => {
    if (!chatState.currentChat) {
      return {};
    }

    const modelId = chatState.currentChat.modelId;
    const providerId = chatState.currentChat.providerId;
    
    if (!modelId || !providerId) {
      return {};
    }

    const model = chatState.allModels.find(m => m.id === modelId && m.provider_id === providerId);

    return {
      modelId,
      providerId,
      model
    };
  });

  // 处理消息发送
  async function handleSendMessage(message: string) {
    console.log('handleSendMessage:', message);
    try {
      if (!chatState.currentChat) {
        console.log('No current chat, creating new chat');
        // 如果没有当前聊天，创建新聊天，但不设置模型，让用户在聊天中选择
        await chatState.createChat();
      }
      console.log('currentChat:', chatState.currentChat);
      await chatState.sendMessage(message);
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
<div class="flex-1 flex flex-col">
  <ChatHeaderView 
    chatId={displayChatId} 
    title={chatTitle}
    sidebarOpen={uiState.sidebarOpen}
  />
  
  <ChatContentView />
  
  <div class="px-4 pb-4">
    <ChatInputView 
      bind:messageInput={messageInput}
      onSendMessage={handleSendMessage}
      selectedModel={currentChatModel().model || null}
    />
  </div>
</div>