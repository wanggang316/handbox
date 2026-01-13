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
}

class FavoriteStore {
  private state = $state<FavoriteState>({
    favorites: [],
    isFavoritedMap: {},
    isLoading: false,
    textRangesByMessageId: {},
    textRangesChatId: null,
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

  getTextRanges(messageId: UUID, chatId?: UUID): TextRange[] {
    if (chatId && this.state.textRangesChatId !== chatId) {
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
        const range = parseTextRange(favorite.content);
        if (!range) continue;
        if (!rangesByMessageId[favorite.messageId]) {
          rangesByMessageId[favorite.messageId] = [];
        }
        rangesByMessageId[favorite.messageId].push(range);
      }

      this.state.textRangesByMessageId = rangesByMessageId;
      this.state.textRangesChatId = chatId;
    } catch (error) {
      console.error("Failed to load text favorites:", error);
    }
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
  }
}

function inferMessageType(content: string): FavoriteMessageType {
  return 'message';
}

function parseTextRange(content: string): TextRange | null {
  try {
    return JSON.parse(content) as TextRange;
  } catch {
    return null;
  }
}

export const favoriteStore = new FavoriteStore();
