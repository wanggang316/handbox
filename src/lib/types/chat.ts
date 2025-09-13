/**
 * 聊天相关类型定义 - 匹配后端 Rust 架构
 */

import type { BaseEntity, UUID, Timestamp } from './index';

// 消息角色
export type MessageRole = 'user' | 'assistant' | 'system';

// 消息配置 - 每条消息可以有独立的配置参数
export interface MessageConfig {
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  stream?: boolean;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  mcpServers?: string[];
}

// 消息类型
export interface Message extends BaseEntity {
  chatId: UUID;
  role: MessageRole;
  content: string;
  reasoning?: string; // 推理过程内容
  
  // 每条消息的配置参数
  config?: MessageConfig;
  
  // 附件
  attachments?: MessageAttachment[];
  
  // 使用统计和时序信息
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  startTime?: Timestamp;
  endTime?: Timestamp;
  duration?: number;
}

// 消息附件
export interface MessageAttachment {
  id: UUID;
  name: string;
  mimeType: string;
  size: number;
  path: string;
}

// 聊天实体
export interface Chat extends BaseEntity {
  name: string;
  lastMessageAt?: Timestamp;
  messageCount: number;
  
  // Chat-level configuration (default values)
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  stream?: boolean;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  mcpServers: string[];
  
  artifactId?: UUID;
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
  chatId?: UUID;
  artifactId?: UUID;
  modelId: string;
  providerId: string;
  parameters?: ModelParameters;
  messages: ChatMessage[];
  attachments?: ChatAttachment[];
}

// 聊天消息（请求中使用）
export interface ChatMessage {
  role: MessageRole;
  content: string;
  reasoning?: string; // 推理过程内容
}

// 聊天附件（请求中使用）
export interface ChatAttachment {
  name: string;
  mimeType: string;
  data: Uint8Array;
}

// 聊天响应
export interface ChatResponse {
  chatId: UUID;
  messageId: UUID;
  content: string;
  modelId: string;
  providerId: string;
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  duration?: number;
}

// 流式聊天事件
export type ChatStreamEvent = 
  | { type: 'delta'; data: { content: string; reasoning?: string; tokens?: number } }
  | { type: 'done'; data: ChatResponse }
  | { type: 'error'; data: { error: string; code?: string } };