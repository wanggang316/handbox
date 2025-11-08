/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type { Chat, UUID, McpServerConfig } from "../types";
import type {
  ModelParameterResponse,
  ModelWithProvider,
} from "../types/provider";
import * as chatApi from "../api/chat";
import {
  providerActions,
  getAllModels,
  providerState,
} from "./provider.svelte";

// ============================================
// 模型参数管理 - 共享工具函数和常量
// ============================================

function getMethodParameters(model?: ModelWithProvider) {
  if (!model?.chat_method || !Array.isArray(model.chat_method.parameters)) {
    return [];
  }

  // 过滤掉无效的参数定义，并标准化名称
  return model.chat_method.parameters
    .filter(
      (param): param is ModelParameterResponse =>
        !!param &&
        typeof param.name === "string" &&
        param.name.trim().length > 0,
    )
    .map((param) => ({
      ...param,
      name: param.name.trim(),
    }));
}

function findMethodParameter(parameterName: string, model?: ModelWithProvider) {
  const target = typeof parameterName === "string" ? parameterName.trim() : "";
  if (!target) {
    return null;
  }

  const params = getMethodParameters(model);
  if (params.length === 0) {
    return null;
  }

  return params.find((param) => param.name === target) ?? null;
}

function getLegacyParameterDefault(
  parameterName: string,
  model?: ModelWithProvider,
): number | undefined {
  if (!model || !Array.isArray(model.parameters)) {
    return undefined;
  }

  const entry = model.parameters.find(
    (param) =>
      typeof param?.name === "string" && param.name.trim() === parameterName,
  );

  if (!entry) {
    return undefined;
  }

  const parsed = toNumber(entry.default ?? null);
  return parsed !== null ? parsed : undefined;
}

function getParameterDefaultNumber(
  parameterName: string,
  model?: ModelWithProvider,
): number | undefined {
  const entry = findMethodParameter(parameterName, model);
  if (entry?.props && "default" in entry.props) {
    const parsed = toNumber(entry.props.default ?? null);
    if (parsed !== null) {
      return parsed;
    }
  }

  return getLegacyParameterDefault(parameterName, model);
}

/**
 * 将值转换为数字
 */
export function toNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/**
 * 将值转换为布尔值
 */
export function toBoolean(value: unknown): boolean | null {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes"].includes(normalized)) return true;
    if (["false", "0", "no"].includes(normalized)) return false;
  }
  return null;
}

/**
 * 获取模型支持的参数集合
 */
export function getSupportedParameterSet(
  model?: ModelWithProvider,
): Set<string> {
  const supported = new Set<string>();

  if (!model) {
    return supported;
  }

  for (const param of getMethodParameters(model)) {
    const name = param.name;
    if (param.support === false) {
      continue;
    }
    if (name) {
      supported.add(name);
    }
  }

  const supportList = Array.isArray(model.supported_parameters)
    ? model.supported_parameters
    : [];

  for (const raw of supportList) {
    if (typeof raw === "string" && raw.trim().length > 0) {
      supported.add(raw.trim());
    }
  }

  if (supported.size === 0 && Array.isArray(model.parameters)) {
    for (const param of model.parameters) {
      const name = typeof param?.name === "string" ? param.name.trim() : "";
      if (name) {
        supported.add(name);
      }
    }
  }

  return supported;
}

/**
 * 检查模型是否支持指定参数
 */
export function hasParameterSupport(
  parameterName: string,
  model?: ModelWithProvider,
): boolean {
  const target = typeof parameterName === "string" ? parameterName.trim() : "";
  if (!target) {
    return false;
  }

  const supportedParameters = getSupportedParameterSet(model);
  if (supportedParameters.size > 0 && supportedParameters.has(target)) {
    return true;
  }

  console.log("supportedParameters: ", supportedParameters);

  const entry = findMethodParameter(target, model);
  if (entry) {
    if (typeof entry.support === "boolean") {
      return entry.support;
    }

    if (entry.props) {
      const hasValue =
        ("default" in entry.props &&
          toNumber(entry.props.default ?? null) !== null) ||
        ("max" in entry.props && toNumber(entry.props.max ?? null) !== null) ||
        ("min" in entry.props && toNumber(entry.props.min ?? null) !== null);

      if (hasValue) {
        return true;
      }
    }
  }

  // 如果没有明确的参数支持信息，但模型提供了全局支持列表，则尝试匹配
  const supportedList = Array.isArray(model?.supported_parameters)
    ? model.supported_parameters
    : null;
  if (supportedParameters.size === 0 && supportedList) {
    if (
      supportedList.some(
        (value) => typeof value === "string" && value.trim() === target,
      )
    ) {
      return true;
    }
  }

  return false;
}

/**
 * 获取参数的最大值
 */
export function getMaxNumber(
  parameterName: string,
  fallback: number,
  model?: ModelWithProvider,
): number {
  const entry = findMethodParameter(parameterName, model);
  if (entry?.props && "max" in entry.props) {
    const parsed = toNumber(entry.props.max ?? null);
    if (parsed !== null) {
      return parsed;
    }
  }

  return fallback;
}

/**
 * 确保值是有效数字
 */
export function ensureNumber(
  value: number | null | undefined,
  fallback: number,
): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/**
 * 将值限制在指定范围内
 */
export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  if (!Number.isFinite(max) || max <= min) return value < min ? min : value;
  return Math.min(Math.max(value, min), max);
}

/**
 * 获取模型的默认设置
 */
export function getModelDefaultSettings(model?: ModelWithProvider) {
  const temperature = getParameterDefaultNumber("temperature", model);
  const topP = getParameterDefaultNumber("top_p", model);
  const topK = hasParameterSupport("top_k", model)
    ? getParameterDefaultNumber("top_k", model)
    : undefined;

  const streamingEntry = hasParameterSupport("streaming", model)
    ? findMethodParameter("streaming", model)
    : null;

  const streamResponse = streamingEntry
    ? (() => {
        const parsed =
          streamingEntry.props && "default" in streamingEntry.props
            ? toBoolean(streamingEntry.props.default ?? null)
            : null;
        return parsed !== null ? parsed : undefined;
      })()
    : undefined;

  const maxTokensCandidate =
    getParameterDefaultNumber("output_max_tokens", model) ??
    getParameterDefaultNumber("max_tokens", model) ??
    (typeof model?.output_max_tokens === "number" &&
    Number.isFinite(model.output_max_tokens)
      ? model.output_max_tokens
      : undefined);

  return {
    temperature,
    topP,
    topK:
      typeof topK === "number" && Number.isFinite(topK)
        ? Math.max(topK, 1)
        : undefined,
    streamResponse,
    maxTokens:
      typeof maxTokensCandidate === "number" &&
      Number.isFinite(maxTokensCandidate) &&
      maxTokensCandidate > 0
        ? maxTokensCandidate
        : undefined,
  };
}

// ============================================
// 聊天状态 - 使用 Svelte 5 runes
// ============================================
let currentChat = $state<Chat | null>(null);
let chats = $state<Chat[]>([]);
let isLoading = $state(false);
let chatError = $state<string | null>(null);
let isInitialized = $state(false);
let isInitializing = $state(false);

export const chatState = {
  get currentChat() {
    return currentChat;
  },
  set currentChat(value) {
    currentChat = value;
  },

  get chats() {
    return chats;
  },
  set chats(value) {
    chats = value;
  },

  get isLoading() {
    return isLoading;
  },
  set isLoading(value) {
    isLoading = value;
  },

  get chatError() {
    return chatError;
  },
  set chatError(value) {
    chatError = value;
  },

  get isInitialized() {
    return isInitialized;
  },
  set isInitialized(value) {
    isInitialized = value;
  },

  get isInitializing() {
    return isInitializing;
  },
  set isInitializing(value) {
    isInitializing = value;
  },
};

// 派生状态：是否有活跃聊天
export function hasActiveChat(): boolean {
  return currentChat !== null && currentChat?.id !== undefined;
}

// 派生状态：当前聊天的模型信息
export function currentChatModel(): { model?: ModelWithProvider } {
  console.log("currentChatModel >>> :", currentChat);
  if (!currentChat) {
    return {};
  }

  const modelId = currentChat.modelId;
  const providerId = currentChat.providerId;

  if (!modelId || !providerId) {
    return {};
  }

  const model = getAllModels().find(
    (m) => m.id === modelId && m.provider_id === providerId,
  );

  return {
    model,
  };
}

// 聊天操作函数
export const chatActions = {
  /**
   * 更新系统提示词
   */
  async updateSystemPrompt(systemPrompt: string): Promise<void> {
    // TODO: 实现单独的 updateChatSystemPrompt API
    if (!chatState.currentChat?.id) {
      return;
    }
    // 暂时直接更新本地状态，后续添加后端 API
    chatState.currentChat.systemPrompt = systemPrompt;
  },

  /**
   * 更新单个模型参数字段
   */
  async updateModelField(
    fieldName:
      | "temperature"
      | "topP"
      | "topK"
      | "maxTokens"
      | "stream"
      | "turnCount",
    value: number | boolean | null,
  ): Promise<void> {
    if (!chatState.currentChat?.id) {
      return;
    }

    // topK 暂不支持后端存储，跳过
    if (fieldName === "topK") {
      console.warn("topK parameter is not yet supported in backend");
      return;
    }

    const updated = await chatApi.updateChatField(
      chatState.currentChat.id,
      fieldName as
        | "temperature"
        | "topP"
        | "maxTokens"
        | "stream"
        | "turnCount",
      value,
    );

    // 更新本地状态
    chatState.currentChat = updated;
  },

  /**
   * 更新模型参数（批量）
   * @deprecated 建议使用 updateModelField 进行单字段更新
   */
  async updateModelSettings(settings: {
    temperature?: number | null;
    topP?: number | null;
    topK?: number | null;
    maxTokens?: number | null;
    stream?: boolean | null;
  }): Promise<void> {
    if (!chatState.currentChat?.id) {
      return;
    }

    let updated = chatState.currentChat;

    // 更新每个提供的字段
    if (settings.temperature !== undefined) {
      updated = await chatApi.updateChatField(
        chatState.currentChat.id,
        "temperature",
        settings.temperature,
      );
    }
    if (settings.topP !== undefined) {
      updated = await chatApi.updateChatField(
        chatState.currentChat.id,
        "topP",
        settings.topP,
      );
    }
    if (settings.maxTokens !== undefined) {
      updated = await chatApi.updateChatField(
        chatState.currentChat.id,
        "maxTokens",
        settings.maxTokens,
      );
    }
    if (settings.stream !== undefined) {
      updated = await chatApi.updateChatField(
        chatState.currentChat.id,
        "stream",
        settings.stream,
      );
    }

    // 更新本地状态
    chatState.currentChat = updated;
  },

  /**
   * 更新MCP服务器配置
   */
  async updateMcpServers(mcpServers: McpServerConfig[]): Promise<void> {
    // TODO: 实现单独的 updateChatMcpServers API
    if (!chatState.currentChat?.id) {
      return;
    }
    // 暂时直接更新本地状态，后续添加后端 API
    chatState.currentChat.mcpServers = mcpServers;
  },

  /**
   * 更新当前聊天的模型信息并保存到后端
   */
  async updateChatModel(modelId: string, providerId: string): Promise<void> {
    // 如果没有 currentChat，创建一个临时的 chat 对象来保存模型选择
    // 实际的数据库保存会在用户发送消息时通过 createChat 完成
    console.log("modelid: {}, providerId: {}", modelId, providerId);
    if (!currentChat) {
      console.log(
        "No current chat, creating temporary chat object for model selection",
      );
      currentChat = {
        name: "未命名",
        messageCount: 0,
        modelId,
        providerId,
        mcpServers: [],
        createdAt: Date.now(),
        updatedAt: Date.now(),
      } as Chat;

      console.log("Temporary chat object created:", currentChat);
      return;
    }

    // 更新本地状态
    currentChat.modelId = modelId;
    currentChat.providerId = providerId;

    // 如果已经有 id（已保存到后端），则更新后端
    if (currentChat.id) {
      try {
        await chatApi.updateChatModel(currentChat.id, modelId, providerId);
      } catch (error) {
        // 回滚本地状态
        await chatActions.loadChats(); // 重新加载以恢复状态
        throw error;
      }
    }
  },

  /**
   * 加载聊天列表
   */
  async loadChats(): Promise<void> {
    try {
      isLoading = true;
      const chatList = await chatApi.getChats();
      chats = chatList;
    } catch (error) {
      chatError = error instanceof Error ? error.message : "加载聊天列表失败";
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 创建新聊天
   */
  async createChat(name: string): Promise<Chat> {
    try {
      if (!currentChat) {
        throw new Error("没有当前聊天");
      }

      if (!currentChat.modelId || !currentChat.providerId) {
        throw new Error("没有设置模型信息");
      }

      isLoading = true;

      console.log("Creating chat with current configuration:", currentChat);
      // 使用当前配置创建聊天，未设置的参数交由后端默认处理
      const chat = await chatApi.createChat(
        name,
        currentChat.temperature,
        currentChat.topP,
        currentChat.maxTokens,
        currentChat.stream,
        currentChat.modelId,
        currentChat.providerId,
        currentChat.systemPrompt,
        currentChat.mcpServers,
      );

      // 更新聊天列表（归一化为数组后再拼接，避免展开不可迭代对象）
      const currentChats = Array.isArray(chats) ? chats : [];
      chats = [chat, ...currentChats];

      // 设置为当前聊天
      currentChat = chat;

      console.log("Created chat:", chat);
      return chat;
    } catch (error) {
      chatError = error instanceof Error ? error.message : "创建聊天失败";
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 切换到指定聊天
   */
  async switchToChat(chatId: UUID): Promise<void> {
    try {
      isLoading = true;

      // 确保模型数据已经加载完成
      if (providerState.providersWithModels.length === 0) {
        await providerActions.loadProvidersWithModels();
      }

      const chat = await chatApi.getChat(chatId);
      currentChat = chat;
    } catch (error) {
      chatError = error instanceof Error ? error.message : "切换聊天失败";
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    chatError = null;
  },

  /**
   * 初始化状态
   */
  async initialize(): Promise<void> {
    // 避免重复初始化
    if (isInitialized || isInitializing) {
      return;
    }

    try {
      isInitializing = true;

      await Promise.all([
        providerActions.loadProvidersWithModels(),
        chatActions.loadChats(),
      ]);

      isInitialized = true;
      console.log("Chat state initialized successfully");
    } catch (error) {
      console.error("Failed to initialize chat state:", error);
    } finally {
      isInitializing = false;
    }
  },

  /**
   * 删除聊天
   */
  async deleteChat(chatId: UUID): Promise<void> {
    try {
      isLoading = true;

      // 调用后端删除聊天
      await chatApi.deleteChat(chatId);

      // 从本地状态中移除聊天
      chats = chats.filter((chat) => chat.id !== chatId);

      // 如果删除的是当前聊天，清空当前聊天状态
      if (currentChat?.id === chatId) {
        currentChat = null;
      }

      console.log("Chat deleted:", chatId);
    } catch (error) {
      chatError = error instanceof Error ? error.message : "删除聊天失败";
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 重命名聊天（包括手动重命名和自动标题生成）
   */
  async renameChat(chatId: UUID, newName: string): Promise<void> {
    try {
      // 调用后端更新聊天名称
      const updatedChat = await chatApi.updateChatName(chatId, newName);

      // 更新本地状态
      const chatIndex = chats.findIndex((chat) => chat.id === chatId);
      if (chatIndex !== -1) {
        chats[chatIndex] = updatedChat;
      }

      // 如果是当前聊天，也更新当前聊天状态
      if (currentChat?.id === chatId) {
        currentChat = updatedChat;
      }

      console.log("Chat renamed:", chatId, newName);
    } catch (error) {
      chatError = error instanceof Error ? error.message : "重命名聊天失败";
      throw error;
    }
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    currentChat = null;
    chats = [];
    isLoading = false;
    chatError = null;

    isInitialized = false;
    isInitializing = false;
  },
};
