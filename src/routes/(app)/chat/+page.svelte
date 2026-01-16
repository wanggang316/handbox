<script lang="ts">
  import ChatHeaderView from "$lib/components/chat/ChatHeader.svelte";
  import ChatContentView from "$lib/components/chat/ChatContent.svelte";
  import ChatInputView from "$lib/components/chat/ChatInput.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import {
    chatState,
    chatActions,
    hasActiveChat,
    currentChatModel,
  } from "$lib/states/chat.svelte";
  import {
    setupProvidersUpdatedListener,
    cleanupProvidersUpdatedListener,
  } from "$lib/states/provider.svelte";
  import { mcpState } from "$lib/states/mcp.svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import * as chatApi from "$lib/api/chat";
  import { goto } from "$app/navigation";
  import { messageStore } from "$lib/states";
  import type { ChatAttachment } from "$lib/types/chat";

  let chatId = $state("");
  let messageInput = $state("");
  let editingMessageId = $state<string | null>(null);
  let targetMessageId = $state<string | null>(null);
  let messageFocusKey = $state<string | null>(null);

  // 已关闭 MCP 提示相关状态
  let showDisabledMcpWarning = $state(false);
  let disabledMcpServers = $state<
    Array<{ serverId: string; name: string; reason: string }>
  >([]);
  let pendingMessage = $state<string | null>(null);

  // 从 URL 参数获取聊天 ID
  onMount(() => {
    // 注册跨窗口事件监听器（用于同步供应商/模型状态）
    setupProvidersUpdatedListener().catch((error) => {
      console.error("Failed to setup providers updated listener:", error);
    });

    // 异步初始化
    (async () => {
      // 确保 chatState 已经初始化
      if (!chatState.isInitialized && !chatState.isInitializing) {
        await chatActions.initialize();
      }

      const urlParams = $page.url.searchParams;
      const newChatId = urlParams.get("id") || "";
      targetMessageId = urlParams.get("message");
      messageFocusKey = targetMessageId
        ? `${targetMessageId}:${urlParams.get("focus") ?? ""}`
        : null;

      // 如果有 chatId，切换到对应聊天
      if (newChatId && newChatId !== chatId) {
        chatId = newChatId;
        try {
          await chatActions.switchToChat(chatId);
        } catch (error: any) {
          console.error("Failed to switch to chat:", error);
        }
      } else if (!newChatId) {
        // 清空当前聊天状态，显示默认界面
        chatId = "";
        chatState.currentChat = null;
      }
    })();

    // 清理函数
    return () => {
      cleanupProvidersUpdatedListener();
    };
  });

  // 监听 URL 变化
  $effect(() => {
    const urlParams = $page.url.searchParams;
    const newChatId = urlParams.get("id") || "";
    const newMessageId = urlParams.get("message");
    const newFocusKey = newMessageId
      ? `${newMessageId}:${urlParams.get("focus") ?? ""}`
      : null;

    if (newChatId !== chatId) {
      chatId = newChatId;
      console.log("Chat ID changed to:", chatId);

      if (chatId) {
        chatActions.switchToChat(chatId).catch((error) => {
          console.error("Failed to switch to chat:", error);
        });
      } else {
        // 清空当前聊天状态，显示默认界面
        chatState.currentChat = null;
      }
    }

    targetMessageId = newMessageId;
    messageFocusKey = newFocusKey;
  });


  // 派生状态：当前聊天信息
  let currentChat = $derived(chatState.currentChat);

  // 派生状态：聊天标题和 ID
  let chatTitle = $derived(currentChat ? currentChat.name : "HandBox");
  let displayChatId = $derived(currentChat ? currentChat.id : "");

  // 当前聊天的模型信息已通过导入的 currentChatModel 提供

  // 处理消息编辑
  function handleEditMessage(messageId: string, content: string) {
    console.log("handleEditMessage:", { messageId, content });
    editingMessageId = messageId;
    messageInput = content;
  }

  // 取消编辑
  function handleCancelEdit() {
    console.log("handleCancelEdit");
    editingMessageId = null;
    messageInput = "";
  }

  // 检测已关闭的 MCP 服务器
  function checkDisabledMcpServers(): Array<{
    serverId: string;
    name: string;
    reason: string;
  }> {
    const currentServers = chatState.currentChat?.mcpServers || [];
    if (currentServers.length === 0) return [];

    const disabled = currentServers
      .map((config) => {
        const server = mcpState.servers.find((s) => s.id === config.serverId);

        // 服务器存在但未启用
        if (server && !server.enabled) {
          return {
            serverId: config.serverId,
            name: server.displayName || server.name,
            reason: "服务器已关闭",
          };
        }

        // 服务器状态不是 ready
        if (server && server.status !== "ready") {
          return {
            serverId: config.serverId,
            name: server.displayName || server.name,
            reason: "服务器未就绪",
          };
        }

        // 服务器已被删除
        if (!server) {
          return {
            serverId: config.serverId,
            name: config.serverId,
            reason: "服务器已删除",
          };
        }

        return null;
      })
      .filter((item) => item !== null);

    return disabled;
  }

  // 处理消息发送
  async function handleSendMessage(
    message: string,
    attachments: ChatAttachment[],
  ) {
    console.log("handleSendMessage:", { message, editingMessageId });

    // 检测已关闭的 MCP 服务器（仅在非编辑模式下检测）
    if (!editingMessageId) {
      const disabled = checkDisabledMcpServers();
      if (disabled.length > 0) {
        disabledMcpServers = disabled;
        pendingMessage = message;
        showDisabledMcpWarning = true;
        // 恢复输入框内容
        messageInput = message;
        return;
      }
    }

    // 实际发送消息
    await sendMessageInternal(message, attachments);
  }

  // 实际发送消息的内部方法
  async function sendMessageInternal(
    message: string,
    attachments: ChatAttachment[],
  ) {
    try {
      // 如果是编辑模式，调用 resendMessage
      if (editingMessageId) {
        if (!chatId) {
          throw new Error("未选择聊天会话");
        }
        await messageStore.resendMessage(chatId, editingMessageId, message);
        // 清除编辑状态
        editingMessageId = null;
        return;
      }

      // 否则是新消息
      if (!hasActiveChat()) {
        console.log("No active chat, creating new chat");
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
              console.error("Failed to generate title:", error);
            }
          }, 100); // 给一点延迟确保消息先发送
        }
      }

      // 使用简化的 messageStore 发送消息
        await messageStore.sendMessage(message, attachments);
    } catch (error) {
      console.error("Failed to send message:", error);
      // 如果是模型选择错误，可以在这里显示提示
      if (error instanceof Error && error.message.includes("选择模型")) {
        // TODO: 显示模型选择提示或自动打开模型选择弹框
        console.log("Model selection required");
      }
    }
  }

  // 打开聊天设置抽屉
  function handleOpenChatSettings() {
    showDisabledMcpWarning = false;
    // 恢复输入框内容
    if (pendingMessage) {
      messageInput = pendingMessage;
    }
    uiState.openModal("chat-settings");
  }

  // 取消发送
  function handleCancelSend() {
    showDisabledMcpWarning = false;
    // 恢复输入框内容
    if (pendingMessage) {
      messageInput = pendingMessage;
    }
    pendingMessage = null;
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
    <ChatContentView
      onEditMessage={handleEditMessage}
      {targetMessageId}
      focusKey={messageFocusKey}
    />
  </div>

  <!-- 固定在底部的输入区域 -->
  <div class="flex-shrink-0 px-4 pb-4">
    <ChatInputView
      bind:messageInput
      bind:editingMessageId
      onSendMessage={handleSendMessage}
      onCancelEdit={handleCancelEdit}
    />
  </div>
</div>

<!-- 已关闭 MCP 提示弹窗 -->
<ConfirmModal
  open={showDisabledMcpWarning}
  title="检测到已关闭的 MCP 服务器"
  message={`当前聊天配置中有 <span class='font-medium'>${disabledMcpServers.length}</span> 个 MCP 服务器在全局设置中被关闭：<br/>${disabledMcpServers.map((s) => `<span class='text-warning'>• ${s.name}</span> - ${s.reason}`).join("<br/>")}`}
  confirmText="设置"
  cancelText="取消"
  confirmButtonStyle="primary"
  onClose={() => (showDisabledMcpWarning = false)}
  onConfirm={handleOpenChatSettings}
  onCancel={handleCancelSend}
/>
