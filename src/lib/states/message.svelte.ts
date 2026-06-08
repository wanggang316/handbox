/**
 * 消息状态管理 - 使用 Svelte 5 响应式最佳实践
 */

import type {
  Message,
  MessageResponse,
  MessageRequest,
  ChatAttachment,
  MessageRequestAttachment,
  ToolCall,
  ToolExecutionStatus,
  UserMessageSendRequest,
} from "$lib/types/chat";
import type { ProviderConfig, UUID } from "$lib/types";
import * as messageApi from "$lib/api/message";
import { listenToStreamEvents } from "$lib/api/message";
import {
  getProviderConfigById,
  getProviderConfig as getProviderConfigByType,
} from "./provider.svelte";
import { chatState } from "./chat.svelte";
import { showAppError, type ErrorDisplayOptions } from "$lib/utils";

interface MessageState {
  // 按 chatId 组织消息
  messagesByChat: Record<string, Message[]>;
  // providerId 到 providerConfig 的映射字典（用于快速获取 provider 图标等信息）
  providerConfigsCache: Record<string, ProviderConfig>;
  isLoading: boolean;
  isSending: boolean;
  error: string | null;
  // 流式响应状态
  streamingMessageId: string | null;
  streamingContent: string;
  streamingReasoning: string;
  streamingToolCalls: ToolCall[] | null;
  streamingIsGeneratingAssets: boolean;
}

interface ToolExecuteEventPayload {
  messageId: string;
  toolCalls?: Record<string, Partial<ToolCall>>;
}

class MessageStore {
  private state = $state<MessageState>({
    messagesByChat: {},
    providerConfigsCache: {},
    isLoading: false,
    isSending: false,
    error: null,
    streamingMessageId: null,
    streamingContent: "",
    streamingReasoning: "",
    streamingToolCalls: null,
    streamingIsGeneratingAssets: false,
  });

  // 当前流式事件监听器的清理函数
  private currentStreamUnlisten: (() => void) | null = null;

  // Getters
  get isLoading() {
    return this.state.isLoading;
  }

  get isSending() {
    return this.state.isSending;
  }

  get error() {
    return this.state.error;
  }

  get streamingMessageId() {
    return this.state.streamingMessageId;
  }

  get streamingContent() {
    return this.state.streamingContent;
  }

  get streamingReasoning() {
    return this.state.streamingReasoning;
  }

  get streamingToolCalls() {
    return this.state.streamingToolCalls;
  }

  get streamingIsGeneratingAssets() {
    return this.state.streamingIsGeneratingAssets;
  }

  // 判断是否正在推理中（有推理内容但还没有最终内容）
  get isReasoning() {
    return this.state.streamingReasoning && !this.state.streamingContent;
  }

  // 判断是否在等待消息响应（发送中但还没有任何流式内容）
  get isMessageLoading() {
    return (
      this.state.isSending &&
      !this.state.streamingReasoning &&
      !this.state.streamingContent
    );
  }

  // 响应式getter用于UI绑定 - 直接返回内部状态以确保响应性
  getMessagesReactive(chatId: string) {
    return this.state.messagesByChat[chatId] || [];
  }

  // 获取当前聊天的消息（通过外部传入 chatId）
  getCurrentMessages(currentChatId: string | undefined): Message[] {
    return currentChatId ? this.getMessages(currentChatId) : [];
  }

  // 根据 providerId 获取 providerConfig（带缓存）
  getProviderConfig(providerId: string): ProviderConfig | undefined {
    // 先从缓存中查找
    if (this.state.providerConfigsCache[providerId]) {
      return this.state.providerConfigsCache[providerId];
    }

    // 缓存中没有，从 providerState 中获取
    let config = getProviderConfigById(providerId);
    if (!config) {
      config = getProviderConfigByType(providerId);
    }
    if (config) {
      // 缓存结果
      this.state.providerConfigsCache[providerId] = config;
      return config;
    }

    return undefined;
  }

  // 根据 providerId 获取 provider 图标（统一走 models.dev 远程 SVG，不再用本地图标）
  getProviderIcon(providerId: string): string | undefined {
    const config = this.getProviderConfig(providerId);
    return config?.provider_type
      ? `https://models.dev/logos/${config.provider_type}.svg`
      : undefined;
  }

  // 批量缓存 providerConfigs（在加载消息时调用）
  private cacheProviderConfigs(messages: Message[]): void {
    const providerIds = new Set(
      messages.map((m) => m.config?.providerId).filter(Boolean) as string[],
    );

    for (const providerId of providerIds) {
      if (!this.state.providerConfigsCache[providerId]) {
        const config = getProviderConfigById(providerId);
        if (config) {
          this.state.providerConfigsCache[providerId] = config;
        }
      }
    }
  }

  private applyMessageResponse(
    chatId: string,
    response: MessageResponse,
  ): void {
    if (!this.state.messagesByChat[chatId]) {
      this.state.messagesByChat[chatId] = [];
    }

    const messages = this.state.messagesByChat[chatId];
    const index = messages.findIndex(
      (message) => message.id === response.messageId,
    );

    if (index !== -1) {
      const existing = messages[index];

      const updated: Message = {
        ...existing,
        content: response.content,
        reasoning: response.reasoning,
        toolCalls: response.toolCalls,
        generatedAssets: response.generatedAssets,
        inputTokens: response.inputTokens,
        outputTokens: response.outputTokens,
        totalTokens: response.totalTokens,
        duration: response.duration,
        updatedAt: Date.now(),
      };

      messages[index] = updated;
    } else {
      const newMessage: Message = {
        id: response.messageId,
        sessionId: chatId,
        role: "assistant",
        content: response.content,
        reasoning: response.reasoning,
        toolCalls: response.toolCalls,
        generatedAssets: response.generatedAssets,
        config: {
          modelId: response.modelId,
          providerId: response.providerId,
          stream: false,
        },
        inputTokens: response.inputTokens,
        outputTokens: response.outputTokens,
        totalTokens: response.totalTokens,
        duration: response.duration,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };

      this.addMessage(chatId, newMessage);
    }
  }

  // 获取指定聊天的消息
  getMessages(chatId: string): Message[] {
    return this.state.messagesByChat[chatId] || [];
  }

  // 获取指定消息
  getMessage(chatId: string, messageId: string): Message | null {
    const messages = this.getMessages(chatId);
    return messages.find((m) => m.id === messageId) || null;
  }

  // Actions
  setLoading(loading: boolean) {
    this.state.isLoading = loading;
  }

  setSending(sending: boolean) {
    this.state.isSending = sending;
  }

  setError(error: string | null) {
    this.state.error = error;
  }

  private reportError(
    error: unknown,
    fallbackMessage: string,
    options?: ErrorDisplayOptions,
  ) {
    const normalized = showAppError(error, {
      fallbackMessage,
      ...options,
    });
    this.setError(normalized.message);
    return normalized;
  }

  // 设置聊天的消息列表
  setMessages(chatId: string, messages: Message[]) {
    // 如果正在发送消息且本地已有消息，避免覆盖
    const existingMessages = this.state.messagesByChat[chatId] || [];
    if (
      this.isSending &&
      existingMessages.length > 0 &&
      messages.length === 0
    ) {
      return;
    }

    this.state.messagesByChat[chatId] = messages;
    // 缓存消息中的 providerConfigs
    this.cacheProviderConfigs(messages);
  }

  // 添加消息到指定聊天
  addMessage(chatId: string, message: Message) {
    if (!this.state.messagesByChat[chatId]) {
      this.state.messagesByChat[chatId] = [];
    }
    this.state.messagesByChat[chatId].push(message);
    // 缓存新消息的 providerConfig
    this.cacheProviderConfigs([message]);
  }

  // 根据ID获取消息
  getMessageById(chatId: string, messageId: string): Message | undefined {
    const messages = this.state.messagesByChat[chatId];
    if (messages) {
      return messages.find((m) => m.id === messageId);
    }
    return undefined;
  }

  // 更新消息
  updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
    const messages = this.state.messagesByChat[chatId];
    if (messages) {
      const index = messages.findIndex((m) => m.id === messageId);
      if (index !== -1) {
        const updatedMessage = { ...messages[index], ...updates };
        const nextMessages = [...messages];
        nextMessages[index] = updatedMessage;
        this.state.messagesByChat[chatId] = nextMessages;
      }
    }
  }

  // 删除消息
  deleteMessage(chatId: string, messageId: string) {
    const messages = this.state.messagesByChat[chatId];
    if (messages) {
      this.state.messagesByChat[chatId] = messages.filter(
        (m) => m.id !== messageId,
      );
    }
  }

  // 开始流式响应
  startStreaming(messageId: string) {
    this.state.streamingMessageId = messageId;
    this.state.streamingContent = "";
    this.state.streamingReasoning = "";
    this.state.streamingToolCalls = null;
    this.state.streamingIsGeneratingAssets = false;
  }

  // 更新流式内容
  appendStreamingContent(content: string) {
    this.state.streamingContent = content; // 直接设置完整内容，因为后端发送的是累积内容
  }

  // 设置流式内容
  setStreamingContent(content: string) {
    this.state.streamingContent = content;
  }

  // 设置流式推理过程
  setStreamingReasoning(reasoning: string) {
    this.state.streamingReasoning = reasoning;
  }

  // 设置流式工具调用
  setStreamingToolCalls(toolCalls: ToolCall[] | null) {
    this.state.streamingToolCalls = toolCalls;
  }

  // 设置是否正在生成资源
  setStreamingIsGeneratingAssets(value: boolean) {
    this.state.streamingIsGeneratingAssets = value;
  }

  // 添加流式生成的图片
  // 完成流式响应
  finishStreaming(chatId: string, response: MessageResponse) {
    this.applyMessageResponse(chatId, response);

    // 清理流式状态
    this.state.streamingMessageId = null;
    this.state.streamingContent = "";
    this.state.streamingReasoning = "";
    this.state.streamingToolCalls = null;
    this.state.streamingIsGeneratingAssets = false;
  }

  /**
   * 通用的流式事件处理器
   */
  private createStreamEventHandlers(
    onComplete?: (response: MessageResponse) => void,
    onError?: (error: any) => void,
  ) {
    return {
      onStart: (data: any) => {
        console.log("流式开始:", data);
        this.startStreaming(data.messageId);
      },

      onChunk: (data: any) => {
        console.log("流式数据:", data);
        this.setStreamingContent(data.content);
        if (data.reasoning) {
          // 累积推理过程内容，因为后端发送的是增量内容
          this.state.streamingReasoning += data.reasoning;
        }
        if (data.toolCalls) {
          this.setStreamingToolCalls(data.toolCalls);
        }
        if (data.isGeneratingAssets !== undefined) {
          this.setStreamingIsGeneratingAssets(data.isGeneratingAssets);
        }
      },

      onEnd: (data: any) => {
        console.log("流式完成:", data);

        // 构造 MessageResponse 对象
        const response: MessageResponse = {
          chatId: data.chatId,
          messageId: data.messageId || "",
          content: data.finalContent,
          reasoning: data.finalReasoning,
          modelId: data.modelId,
          providerId: data.providerId,
          toolCalls: data.toolCalls,
          generatedAssets: data.generatedAssets,
          inputTokens: undefined,
          outputTokens: undefined,
          totalTokens: undefined,
          duration: undefined,
        };

        // 使用统一的流式完成方法
        this.finishStreaming(data.chatId, response);

        // 执行自定义完成回调
        if (onComplete) {
          onComplete(response);
        }

        // 清理监听器
        if (this.currentStreamUnlisten) {
          this.currentStreamUnlisten();
          this.currentStreamUnlisten = null;
        }
      },

      onError: (payload: any) => {
        console.error("流式错误:", payload);
        const errorDetail = payload?.error ?? payload;
        this.reportError(errorDetail, "流式响应错误", {
          requiresAcknowledgement: true,
          title: "对话失败",
        });

        // 执行自定义错误回调
        if (onError) {
          onError(payload);
        }

        // 错误时也清理监听器
        if (this.currentStreamUnlisten) {
          this.currentStreamUnlisten();
          this.currentStreamUnlisten = null;
        }
      },
    };
  }

  /**
   * 处理消息删除 - 从状态中移除指定的消息
   */
  private handleMessagesDelete(
    chatId: string,
    messageIds: string[],
    source: string = "unknown",
  ) {
    console.log(`[${source}] 消息被删除:`, { chatId, messageIds });

    // 从状态中删除这些消息
    const messages = this.state.messagesByChat[chatId] || [];
    const filteredMessages = messages.filter(
      (m: Message) => m.id && !messageIds.includes(m.id),
    );
    this.state.messagesByChat[chatId] = filteredMessages;

    console.log(
      `[${source}] 已从 chat ${chatId} 删除 ${messageIds.length} 条消息`,
    );
  }

  /**
   * 创建消息删除回调
   */
  private createMessagesDeleteCallback(source: string = "unknown") {
    return (data: { chatId: string; messageIds: string[] }) => {
      this.handleMessagesDelete(data.chatId, data.messageIds, source);
    };
  }

  /**
   * 处理用户消息保存 - 替换临时ID为真实ID
   */
  private handleUserMessageSaved(
    tempMessageId: string,
    savedMessageId: string,
    chatId: string,
    source: string = "unknown",
  ) {
    console.log(`[${source}] 用户消息已保存，替换ID:`, {
      tempMessageId,
      savedMessageId,
      chatId,
    });

    const messages = this.state.messagesByChat[chatId] || [];
    const messageIndex = messages.findIndex(
      (m: Message) => m.id === tempMessageId,
    );

    if (messageIndex !== -1) {
      messages[messageIndex] = {
        ...messages[messageIndex],
        id: savedMessageId,
      };
      this.state.messagesByChat[chatId] = [...messages];
      console.log(
        `[${source}] 已替换 chat ${chatId} 中的消息ID: ${tempMessageId} -> ${savedMessageId}`,
      );
    } else {
      console.warn(
        `[${source}] 未找到临时消息ID: ${tempMessageId} in chat ${chatId}`,
      );
    }
  }

  /**
   * 创建用户消息保存回调
   */
  private createUserMessageSavedCallback(
    chatId: string,
    source: string = "unknown",
  ) {
    return (data: { tempMessageId: string; savedMessageId: string }) => {
      this.handleUserMessageSaved(
        data.tempMessageId,
        data.savedMessageId,
        chatId,
        source,
      );
    };
  }

  // 清理指定聊天的消息
  clearMessages(chatId: string) {
    delete this.state.messagesByChat[chatId];
  }

  // 清理所有消息
  clearAllMessages() {
    this.state.messagesByChat = {};
    this.state.streamingMessageId = null;
    this.state.streamingContent = "";
    this.state.streamingReasoning = "";
    this.state.streamingToolCalls = null;
  }

  // API 操作方法

  /**
   * 加载指定聊天的消息
   */
  async loadMessages(chatId: string): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);
      const messages = await messageApi.getMessages(chatId);
      this.setMessages(chatId, messages);
    } catch (error) {
      this.reportError(error, "加载消息失败");
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 发送消息（使用流式响应）- 简化版本，只需要内容和附件
   */
  async sendMessage(
    content: string,
    attachments: ChatAttachment[],
  ): Promise<void> {
    // 获取当前聊天信息
    const currentChat = chatState.currentChat;
    if (!currentChat || !currentChat.id) {
      throw new Error("没有活跃的聊天");
    }

    if (!currentChat.modelId || !currentChat.providerId) {
      throw new Error(
        "请先为当前聊天选择模型。如果供应商列表为空，请先配置AI供应商。",
      );
    }

    try {
      this.setSending(true);
      this.setError(null);

      const apiAttachments: MessageRequestAttachment[] = attachments.map(
        (attachment) => ({
          name: attachment.name,
          mime_type: attachment.mimeType,
          data: Array.from(attachment.data ?? []),
        }),
      );

      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        sessionId: currentChat.id,
        role: "user",
        content: content,
        config: {
          modelId: currentChat.modelId,
          providerId: currentChat.providerId,
          stream: true,
        },
        createdAt: Date.now(),
        updatedAt: Date.now(),
        attachments: this.createLocalMessageAttachments(attachments),
      };

      this.addMessage(currentChat.id, userMessage);

      // 构建完整的消息请求（不包含 system message，由后端构建）
      const userMessageRequest: UserMessageSendRequest = {
        chatId: currentChat.id,
        content: content,
        tempUserMessageId: userMessage.id || "",
        attachments: apiAttachments,
      };

      // 清理之前的监听器（如果存在）
      if (this.currentStreamUnlisten) {
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }

      // 先设置流式事件监听器，确保在发送消息前完全就绪
      this.currentStreamUnlisten = await messageApi.listenToStreamEvents({
        ...this.createStreamEventHandlers(
          // onComplete callback
          () => {
            this.setSending(false);
          },
          // onError callback
          () => {
            this.setSending(false);
          },
        ),
        onUserMessageSaved: this.createUserMessageSavedCallback(
          currentChat.id,
          "sendMessage",
        ),
      });

      // 事件监听器设置完成后，再发送流式消息
      await messageApi.sendUserMessageStream(userMessageRequest);
    } catch (error) {
      this.reportError(error, "发送消息失败", {
        title: "消息发送失败",
      });
      this.setSending(false);
      throw error;
    }
  }

  private createLocalMessageAttachments(
    attachments: ChatAttachment[],
  ): Message["attachments"] {
    if (!attachments?.length) {
      return undefined;
    }

    return attachments.map((attachment, index) => ({
      id: crypto.randomUUID(),
      name: attachment.name || `附件${index + 1}`,
      mimeType: attachment.mimeType,
      size: attachment.data?.length ?? 0,
      path: this.convertAttachmentToDataUrl(attachment),
    }));
  }

  private convertAttachmentToDataUrl(attachment: ChatAttachment): string {
    const data = attachment.data;
    if (!data || data.length === 0) {
      return "";
    }

    const chunkSize = 0x8000;
    let binary = "";
    for (let i = 0; i < data.length; i += chunkSize) {
      const chunk = data.subarray(i, i + chunkSize);
      binary += String.fromCharCode(...chunk);
    }

    const base64 = btoa(binary);
    const mime = attachment.mimeType || "application/octet-stream";
    return `data:${mime};base64,${base64}`;
  }

  /**
   * 删除消息（API调用）
   */
  async removeMessage(chatId: string, messageId: string): Promise<void> {
    try {
      await messageApi.deleteMessage(messageId);
      this.deleteMessage(chatId, messageId);
    } catch (error) {
      this.reportError(error, "删除消息失败");
      throw error;
    }
  }

  /**
   * 流式重新生成消息 - 删除当前消息，根据本轮消息重新生成
   */
  async regenerateMessage(messageId: string): Promise<void> {
    console.log("[regenerateMessage] 开始重新生成消息:", messageId);

    try {
      this.setSending(true);

      // 清理之前的监听器（如果存在）
      if (this.currentStreamUnlisten) {
        console.log("[regenerateMessage] 清理之前的监听器");
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }

      console.log("[regenerateMessage] 设置事件监听器...");

      // 设置流式事件监听器
      this.currentStreamUnlisten = await messageApi.listenToStreamEvents(
        {
          ...this.createStreamEventHandlers(
            // onComplete callback
            () => {
              this.setSending(false);
            },
            // onError callback
            () => {
              this.setSending(false);
            },
          ),
          // 添加消息删除回调
          onMessagesDelete:
            this.createMessagesDeleteCallback("regenerateMessage"),
        },
        "message_stream",
      );

      console.log("[regenerateMessage] 调用流式 API...");

      // 调用流式 API
      await messageApi.regenerateAssistantMessageStream(messageId as UUID);

      console.log("[regenerateMessage] API 调用成功");
    } catch (error) {
      console.error("[regenerateMessage] 重新生成失败:", error);
      this.reportError(error, "重新生成失败", {
        title: "重新生成失败",
      });
      this.setSending(false);
      throw error;
    }
  }

  /**
   * 重发用户消息 - 删除该消息之后的所有消息，然后重新发送（流式）
   * @param messageId 消息ID
   * @param content 可选的新消息内容，如果提供则更新消息内容后重新发送
   */
  async resendMessage(
    chatId: string,
    messageId: string,
    content?: string,
  ): Promise<void> {
    console.log(
      "[resendMessage] 开始重发消息:",
      messageId,
      content ? "(带新内容)" : "",
    );

    try {
      this.setSending(true);

      if (typeof content === "string") {
        this.updateMessage(chatId, messageId, {
          content,
          updatedAt: Date.now(),
        });
      }

      // 清理之前的监听器（如果存在）
      if (this.currentStreamUnlisten) {
        console.log("[resendMessage] 清理之前的监听器");
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }

      console.log("[resendMessage] 设置事件监听器...");

      // 设置流式事件监听器
      this.currentStreamUnlisten = await messageApi.listenToStreamEvents({
        ...this.createStreamEventHandlers(
          // onComplete callback
          () => {
            this.setSending(false);
          },
          // onError callback
          () => {
            this.setSending(false);
          },
        ),
        // 添加消息删除回调
        onMessagesDelete: this.createMessagesDeleteCallback("resendMessage"),
      });

      console.log("[resendMessage] 调用 resendMessageStream API...");

      // 调用流式重发API，传递可选的新内容
      await messageApi.resendUserMessageStream(messageId as UUID, content);

      console.log("[resendMessage] API 调用成功");
    } catch (error) {
      // 清理监听器
      if (this.currentStreamUnlisten) {
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }
      console.error("重发消息失败:", error);
      this.reportError(error, "重发消息失败", {
        title: "重发失败",
      });
      this.setSending(false);
      throw error;
    }
  }

  /**
   * 执行多个工具调用 - 使用流式API
   */
  async executeToolCalls(
    messageId: string,
    toolCallIds: string[],
  ): Promise<void> {
    console.log("[executeToolCalls] 开始执行工具调用:", {
      messageId,
      toolCallIds,
    });

    if (toolCallIds.length === 0) {
      console.warn("[executeToolCalls] 工具调用列表为空");
      return;
    }

    try {
      this.setError(null);

      // 清理之前的监听器（如果存在）
      if (this.currentStreamUnlisten) {
        console.log("[executeToolCalls] 清理之前的监听器");
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }

      console.log("[executeToolCalls] 设置事件监听器...");

      // 监听工具执行流式事件
      this.currentStreamUnlisten = await listenToStreamEvents(
        {
          ...this.createStreamEventHandlers(),
          // 添加消息删除回调
          onMessagesDelete:
            this.createMessagesDeleteCallback("executeToolCalls"),
          // 添加工具执行状态回调
          onToolExecute: (data: ToolExecuteEventPayload) => {
            console.log("[onToolExecute] 工具执行状态变化:", data);

            // 查找消息所属的 chatId
            let foundChatId: string | undefined;
            let foundMessage: Message | undefined;

            for (const [cid, messages] of Object.entries(
              this.state.messagesByChat,
            )) {
              const msg = messages.find((m) => m.id === data.messageId);
              if (msg) {
                foundChatId = cid;
                foundMessage = msg;
                break;
              }
            }

            if (!foundChatId || !foundMessage) {
              console.warn("[onToolExecute] 未找到消息:", data.messageId);
              return;
            }

            // 更新消息中工具调用的状态
            if (
              foundMessage.toolCalls &&
              data.toolCalls &&
              typeof data.toolCalls === "object"
            ) {
              const updatesMap = new Map<string, Partial<ToolCall>>(
                Object.entries(data.toolCalls).filter(
                  ([key, value]) => typeof value === "object" && value !== null,
                ),
              );

              const updatedToolCalls = foundMessage.toolCalls.map((call) => {
                const callId = call.id || "";
                const update = callId ? updatesMap.get(callId) : undefined;

                if (update) {
                  const executionStatus =
                    update.executionStatus ?? call.executionStatus;
                  const isFinalStatus =
                    executionStatus === "completed" ||
                    executionStatus === "failed";
                  const functionUpdate = update.function;

                  return {
                    ...call,
                    executionStatus,
                    executionMode: update.executionMode ?? call.executionMode,
                    toolType: update.toolType ?? call.toolType,
                    function: functionUpdate
                      ? { ...call.function, ...functionUpdate }
                      : call.function,
                    result: isFinalStatus
                      ? typeof update.result === "string"
                        ? update.result
                        : call.result
                      : undefined,
                  };
                }

                return call;
              });

              this.updateMessage(foundChatId, data.messageId, {
                toolCalls: updatedToolCalls,
              });
            }
          },
        },
        "tool_execute_stream",
      );

      console.log("[executeToolCalls] 事件监听器设置完成，调用后端API...");

      // 调用流式工具执行API
      await messageApi.executeToolCallsStream(messageId, toolCallIds);

      console.log("[executeToolCalls] 后端API调用完成");
    } catch (error) {
      // 清理监听器
      if (this.currentStreamUnlisten) {
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }
      console.error("启动工具执行失败:", error);
      this.reportError(error, "工具执行失败");
      throw error;
    }
  }

  async executeToolCall(messageId: string, toolCallId: string): Promise<void> {
    await this.executeToolCalls(messageId, [toolCallId]);
  }

  /**
   * 批量执行消息中的所有工具调用
   */
  async executeAllToolCalls(
    messageId: string,
    toolCalls: ToolCall[],
  ): Promise<void> {
    console.log("[executeAllToolCalls] 开始:", {
      messageId,
      toolCallsCount: toolCalls.length,
    });

    try {
      const toolCallIds = toolCalls
        .map((toolCall) => toolCall.id)
        .filter((id): id is string => Boolean(id));

      console.log("[executeAllToolCalls] 提取的工具调用IDs:", toolCallIds);

      if (toolCallIds.length === 0) {
        console.warn("[executeAllToolCalls] 未找到有效的工具调用 ID");
        return;
      }

      console.log("[executeAllToolCalls] 调用 executeToolCalls...");
      await this.executeToolCalls(messageId, toolCallIds);
      console.log("[executeAllToolCalls] executeToolCalls 完成");
    } catch (error) {
      console.error("[executeAllToolCalls] 批量执行工具调用失败:", error);
      throw error;
    }
  }

  // 清理所有状态
  clear() {
    this.state.messagesByChat = {};
    this.state.providerConfigsCache = {};
    this.state.isLoading = false;
    this.state.isSending = false;
    this.state.error = null;
    this.state.streamingMessageId = null;
    this.state.streamingContent = "";
    this.state.streamingReasoning = "";
    this.state.streamingToolCalls = null;
  }
}

// Export singleton instance
export const messageStore = new MessageStore();
