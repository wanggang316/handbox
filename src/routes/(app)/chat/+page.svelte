<script lang="ts">
  import ChatHeaderView from '$lib/components/chat/ChatHeaderView.svelte';
  import ChatContentView from '$lib/components/chat/ChatContentView.svelte';
  import ChatInputView from '$lib/components/chat/ChatInputView.svelte';
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { uiState } from '$lib/states/ui.svelte';
  import { chatState } from '$lib/states/chat.svelte';
  import { messageStore } from '$lib/states/message.svelte';

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

  // 派生状态：当前聊天的模型信息（直接使用 chatState 中的 getter）
  let currentChatModel = $derived(chatState.currentChatModel);

  // 处理消息发送
  async function handleSendMessage(message: string) {
    console.log('handleSendMessage:', message);
    try {
      if (!chatState.currentChat) {
        console.log('No current chat, creating new chat');
        // 如果没有当前聊天，创建新聊天，但不设置模型，让用户在聊天中选择
        await chatState.createChat();
      }
      
      const chat = chatState.currentChat;
      if (!chat) {
        throw new Error('没有活跃的聊天');
      }

      if (!chat.modelId || !chat.providerId) {
        throw new Error('请先为当前聊天选择模型。如果供应商列表为空，请先配置AI供应商。');
      }
      
      console.log('currentChat:', chat);
      
      // 使用 messageStore 发送消息
      await messageStore.sendMessage({
        chatId: chat.id,
        modelId: chat.modelId,
        providerId: chat.providerId,
        messages: [{ role: 'user', content: message }],
        attachments: []
      });
      
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
<div class="flex-1 flex flex-col h-full bg-amber-100">
  <!-- 固定在顶部的聊天头部 -->
  <div class="flex-shrink-0">
    <ChatHeaderView 
      chatId={displayChatId} 
      title={chatTitle}
      sidebarOpen={uiState.sidebarOpen}
    />
  </div>
  
  <!-- 可滚动的聊天内容区域，占据剩余空间 -->
  <div class="flex-1 min-h-0 bg-amber-200">
    <ChatContentView />
  </div>
  
  <!-- 固定在底部的输入区域 -->
  <div class="flex-shrink-0 px-4 pb-4">
    <ChatInputView 
      bind:messageInput={messageInput}
      onSendMessage={handleSendMessage}
      selectedModel={currentChatModel.model || null}
    />
  </div>
</div>