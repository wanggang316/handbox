/**
 * 聊天相关类型定义
 */

import type { BaseEntity, UUID, Timestamp } from './index';

// 消息角色
export type MessageRole = 'user' | 'assistant' | 'system';

// 消息类型
export interface Message extends BaseEntity {
  sessionId: UUID;
  role: MessageRole;
  content: string;
  attachments?: MessageAttachment[];
  metadata?: MessageMetadata;
}

// 消息附件
export interface MessageAttachment {
  id: UUID;
  name: string;
  mimeType: string;
  size: number;
  path: string;
}

// 消息元数据
export interface MessageMetadata {
  model?: string;
  provider?: string;
  tokens?: {
    input: number;
    output: number;
    total: number;
  };
  timing?: {
    startTime: number;
    endTime: number;
    duration: number;
  };
  streaming?: boolean;
}

// 聊天会话
export interface ChatSession extends BaseEntity {
  name: string;
  lastMessageAt?: Timestamp;
  messageCount: number;
  config: ChatConfig;
  artifactId?: UUID;
}

// 聊天配置
export interface ChatConfig {
  systemPrompt?: string;
  model: string;
  provider: string;
  parameters: ModelParameters;
  mcpServers: string[];
}

// 模型参数
export interface ModelParameters {
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  contextLength?: number;
  stream?: boolean;
}

// 聊天请求
export interface ChatRequest {
  sessionId?: UUID;
  artifactId?: UUID;
  inlineConfig?: Partial<ChatConfig>;
  messages: Omit<Message, keyof BaseEntity | 'sessionId' | 'metadata'>[];
  attachments?: File[];
}

// 聊天响应
export interface ChatResponse {
  sessionId: UUID;
  messageId: UUID;
  content: string;
  metadata: MessageMetadata;
}

// 流式聊天事件
export type ChatStreamEvent = 
  | { type: 'delta'; data: { content: string; metadata?: Partial<MessageMetadata> } }
  | { type: 'done'; data: ChatResponse }
  | { type: 'error'; data: { error: string; code?: string } };