import type { BaseEntity, UUID } from "./index";

export type FavoriteMessageType = 'text' | 'image' | 'message' | 'chat' | 'external';

export type TagColor = 'primary' | 'secondary' | 'accent' | 'success' | 'warning' | 'error' | 'info' | 'gray';

export interface FavoriteTag {
  name: string;
  color: TagColor;
}

export interface TextRange {
  start: number;
  end: number;
}

export interface SelectionRect {
  x: number;
  y: number;
  width: number;
  height: number;
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
  selectionTextRaw?: string;
  sourceAppName?: string;
  sourceBundleId?: string;
  sourcePid?: number;
  sourceAppPath?: string;
  sourceAppVersion?: string;
  sourceWindowTitle?: string;
  sourceUrl?: string;
  sourceDomain?: string;
  sourceTabTitle?: string;
  selectionRect?: string;
  captureMethod?: string;
  locale?: string;
  inputLanguage?: string;
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

export interface CreateExternalFavoriteDto {
  content: string;
  role: 'user' | 'assistant' | 'system';
  tags?: FavoriteTag[];
  note?: string;
  context?: string;
  selectionTextRaw?: string;
  sourceAppName?: string;
  sourceBundleId?: string;
  sourcePid?: number;
  sourceAppPath?: string;
  sourceAppVersion?: string;
  sourceWindowTitle?: string;
  sourceUrl?: string;
  sourceDomain?: string;
  sourceTabTitle?: string;
  selectionRect?: SelectionRect;
  captureMethod?: string;
  locale?: string;
  inputLanguage?: string;
}
