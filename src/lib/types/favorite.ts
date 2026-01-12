import type { BaseEntity, UUID } from "./index";

export type FavoriteMessageType = 'text' | 'image' | 'message' | 'chat' | 'other';

export interface Favorite extends BaseEntity {
  messageId: UUID;
  chatId: UUID;
  content: string;
  role: 'user' | 'assistant' | 'system';
  messageType: FavoriteMessageType;
  tags: string[];
  note?: string;
}

export interface CreateFavoriteDto {
  messageId: UUID;
  chatId: UUID;
  content: string;
  role: 'user' | 'assistant' | 'system';
  messageType?: FavoriteMessageType;
  tags?: string[];
  note?: string;
}
