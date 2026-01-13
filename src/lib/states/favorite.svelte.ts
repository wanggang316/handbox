/**
 * 收藏状态管理 - 使用 Svelte 5 响应式最佳实践
 */

import type { Favorite, FavoriteMessageType, TextRange } from "$lib/types/favorite";
import type { UUID } from "$lib/types";
import * as favoriteApi from "$lib/api/favorite";

interface FavoriteState {
  favorites: Favorite[];
  isFavoritedMap: Record<string, boolean>;
  isLoading: boolean;
  textRangesByMessageId: Record<string, TextRange[]>;
  textRangesChatId: string | null;
  textRangesVersion: number;
}

class FavoriteStore {
  private state = $state<FavoriteState>({
    favorites: [],
    isFavoritedMap: {},
    isLoading: false,
    textRangesByMessageId: {},
    textRangesChatId: null,
    textRangesVersion: 0,
  });

  get favorites() {
    return this.state.favorites;
  }

  get isLoading() {
    return this.state.isLoading;
  }

  get textRangesByMessageId() {
    return this.state.textRangesByMessageId;
  }

  get textRangesChatId() {
    return this.state.textRangesChatId;
  }

  get textRangesVersion() {
    return this.state.textRangesVersion;
  }

  getTextRanges(messageId: UUID, chatId?: UUID): TextRange[] {
    if (chatId && this.state.textRangesChatId && this.state.textRangesChatId !== chatId) {
      if (import.meta.env.DEV) {
        console.debug("[favoriteStore] text ranges chat mismatch", {
          messageId,
          chatId,
          storeChatId: this.state.textRangesChatId,
        });
      }
      return [];
    }
    return this.state.textRangesByMessageId[messageId] ?? [];
  }

  isFavorited(messageId: string, chatId: string, messageType: FavoriteMessageType): boolean {
    const key = `${messageId}_${chatId}_${messageType}`;
    return this.state.isFavoritedMap[key] ?? false;
  }

  async checkFavorited(messageId: UUID, chatId: UUID, messageType: FavoriteMessageType): Promise<boolean> {
    const key = `${messageId}_${chatId}_${messageType}`;
    const isFavorited = await favoriteApi.isFavorited(messageId, chatId, messageType);
    this.state.isFavoritedMap[key] = isFavorited;
    return isFavorited;
  }

  async toggleFavorite(
    messageId: UUID,
    chatId: UUID,
    content: string,
    role: 'user' | 'assistant' | 'system',
    messageType?: FavoriteMessageType,
    tags?: any[],
    note?: string,
    context?: string,
    updateFavoritedMap: boolean = true,
  ): Promise<boolean> {
    const inferredType = messageType ?? inferMessageType(content);
    try {
      const isFavorited = await favoriteApi.toggleFavorite(
        messageId,
        chatId,
        content,
        role,
        inferredType,
        tags ?? [],
        note,
        context,
      );
      
      // 只有收藏消息类型时才更新isFavoritedMap
      // 文本/图片/对话类型的收藏不应该影响消息的收藏按钮状态
      if (updateFavoritedMap && inferredType === 'message') {
        this.state.isFavoritedMap[messageId] = isFavorited;
      }
      
      if (isFavorited) {
        await this.loadFavorites();
      } else {
        this.state.favorites = this.state.favorites.filter(
          (f) => f.messageId !== messageId,
        );
      }

      if (inferredType === "text") {
        await this.loadTextFavoritesByChat(chatId);
      }
      return isFavorited;
    } catch (error) {
      console.error("Failed to toggle favorite:", error);
      throw error;
    }
  }

  async loadFavorites(): Promise<void> {
    try {
      this.state.isLoading = true;
      const favorites = await favoriteApi.getFavorites();
      this.state.favorites = favorites;
      const map: Record<string, boolean> = {};
      for (const f of favorites) {
        const key = `${f.messageId}_${f.chatId}_${f.messageType}`;
        map[key] = true;
      }
      this.state.isFavoritedMap = map;
    } catch (error) {
      console.error("Failed to load favorites:", error);
    } finally {
      this.state.isLoading = false;
    }
  }

  async loadTextFavoritesByChat(chatId: UUID): Promise<void> {
    try {
      const favorites = await favoriteApi.getFavoritesByChat(chatId);
      const rangesByMessageId: Record<string, TextRange[]> = {};

      for (const favorite of favorites) {
        if (favorite.messageType !== "text") continue;
        if (import.meta.env.DEV) {
          console.debug("[favoriteStore] text favorite raw", {
            messageId: favorite.messageId,
            content: favorite.content,
          });
        }
        const ranges = parseTextRanges(favorite.content);
        if (ranges.length === 0) continue;
        if (!rangesByMessageId[favorite.messageId]) {
          rangesByMessageId[favorite.messageId] = [];
        }
        rangesByMessageId[favorite.messageId].push(...ranges);
      }

      this.state.textRangesByMessageId = normalizeTextRangesMap(rangesByMessageId);
      this.state.textRangesChatId = chatId;
      this.state.textRangesVersion += 1;
      if (import.meta.env.DEV) {
        console.debug("[favoriteStore] text ranges loaded", {
          chatId,
          messageIds: Object.keys(this.state.textRangesByMessageId),
        });
      }
    } catch (error) {
      console.error("Failed to load text favorites:", error);
    }
  }

  async saveTextRanges(
    messageId: UUID,
    chatId: UUID,
    ranges: TextRange[],
    role: 'user' | 'assistant' | 'system',
    context?: string,
  ): Promise<void> {
    const normalized = mergeTextRanges(ranges);
    await favoriteApi.saveTextRanges(messageId, chatId, normalized, role, context);
    if (normalized.length === 0) {
      const next = { ...this.state.textRangesByMessageId };
      delete next[messageId];
      this.state.textRangesByMessageId = next;
    } else {
      this.state.textRangesByMessageId = {
        ...this.state.textRangesByMessageId,
        [messageId]: normalized,
      };
    }
    this.state.textRangesChatId = chatId;
    this.state.textRangesVersion += 1;
    await this.loadFavorites();
  }

  async addTextRange(
    messageId: UUID,
    chatId: UUID,
    range: TextRange,
    role: 'user' | 'assistant' | 'system',
    context?: string,
  ): Promise<void> {
    const existing = this.getTextRanges(messageId, chatId);
    await this.saveTextRanges(messageId, chatId, [...existing, range], role, context);
  }

  async removeTextRange(
    messageId: UUID,
    chatId: UUID,
    target: TextRange,
    role: 'user' | 'assistant' | 'system',
    context?: string,
  ): Promise<void> {
    const existing = this.getTextRanges(messageId, chatId);
    const next = existing.filter(
      (range) => range.start !== target.start || range.end !== target.end,
    );
    await this.saveTextRanges(messageId, chatId, next, role, context);
  }

  async addTag(favoriteId: UUID, tag: string, color: string): Promise<void> {
    try {
      await favoriteApi.addTag(favoriteId, { name: tag, color: color as any });
      await this.loadFavorites();
    } catch (error) {
      console.error("Failed to add tag:", error);
      throw error;
    }
  }

  async removeTag(favoriteId: UUID, tagName: string): Promise<void> {
    try {
      await favoriteApi.removeTag(favoriteId, tagName);
      await this.loadFavorites();
    } catch (error) {
      console.error("Failed to remove tag:", error);
      throw error;
    }
  }

  clear() {
    this.state.favorites = [];
    this.state.isFavoritedMap = {};
    this.state.isLoading = false;
    this.state.textRangesByMessageId = {};
    this.state.textRangesChatId = null;
    this.state.textRangesVersion = 0;
  }
}

function inferMessageType(content: string): FavoriteMessageType {
  return 'message';
}

function parseTextRanges(content: string): TextRange[] {
  try {
    const parsed = JSON.parse(content) as TextRange[] | TextRange;
    if (Array.isArray(parsed)) {
      return parsed;
    }
    if (parsed && typeof parsed === "object") {
      return [parsed];
    }
    if (import.meta.env.DEV) {
      console.debug("[favoriteStore] invalid text range payload", {
        content,
        parsedType: typeof parsed,
      });
    }
    return [];
  } catch (error) {
    if (import.meta.env.DEV) {
      console.debug("[favoriteStore] failed to parse text ranges", {
        content,
        error,
      });
    }
    return [];
  }
}

function mergeTextRanges(ranges: TextRange[]): TextRange[] {
  const normalized = ranges
    .map((range) => ({
      start: Math.max(0, Math.floor(range.start)),
      end: Math.max(0, Math.floor(range.end)),
    }))
    .filter((range) => range.end > range.start)
    .sort((a, b) => a.start - b.start);

  if (normalized.length <= 1) return normalized;

  const merged: TextRange[] = [];
  let current = normalized[0];
  for (let i = 1; i < normalized.length; i += 1) {
    const next = normalized[i];
    if (next.start <= current.end) {
      current = { start: current.start, end: Math.max(current.end, next.end) };
    } else {
      merged.push(current);
      current = next;
    }
  }
  merged.push(current);
  return merged;
}

function normalizeTextRangesMap(
  map: Record<string, TextRange[]>,
): Record<string, TextRange[]> {
  const next: Record<string, TextRange[]> = {};
  for (const [messageId, ranges] of Object.entries(map)) {
    next[messageId] = mergeTextRanges(ranges);
  }
  return next;
}

export const favoriteStore = new FavoriteStore();
