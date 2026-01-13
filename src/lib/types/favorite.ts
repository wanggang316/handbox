import type { BaseEntity, UUID } from "./index";

export type FavoriteMessageType = 'text' | 'image' | 'message' | 'chat';

export type TagColor = 'primary' | 'secondary' | 'accent' | 'success' | 'warning' | 'error' | 'info' | 'gray';

export interface FavoriteTag {
  name: string;
  color: TagColor;
}

export interface TextRange {
  start: number;
  end: number;
}

export interface Favorite {
  id?: UUID;
  messageId: UUID;
  chatId: UUID;
  content: string;
  role: 'user' | 'assistant' | 'system';
  messageType: FavoriteMessageType;
  tags: FavoriteTag[];
  note?: string;
  context?: string;
  createdAt: number;
  updatedAt: number;
}

export interface CreateFavoriteDto {
  messageId: UUID;
  chatId: UUID;
  content: string;
  role: 'user' | 'assistant' | 'system';
  messageType?: FavoriteMessageType;
  tags?: FavoriteTag[];
  note?: string;
  context?: string;
}
