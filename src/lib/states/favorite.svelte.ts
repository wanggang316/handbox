/**
 * 收藏状态管理 - 使用 Svelte 5 响应式最佳实践
 */

import type { Favorite, FavoriteMessageType } from "$lib/types/favorite";
import type { UUID } from "$lib/types";
import * as favoriteApi from "$lib/api/favorite";

interface FavoriteState {
  favorites: Favorite[];
  isFavoritedMap: Record<string, boolean>;
  isLoading: boolean;
}

class FavoriteStore {
  private state = $state<FavoriteState>({
    favorites: [],
    isFavoritedMap: {},
    isLoading: false,
  });

  get favorites() {
    return this.state.favorites;
  }

  get isLoading() {
    return this.state.isLoading;
  }

  isFavorited(messageId: string): boolean {
    return this.state.isFavoritedMap[messageId] ?? false;
  }

  async toggleFavorite(
    messageId: UUID,
    chatId: UUID,
    content: string,
    role: 'user' | 'assistant' | 'system',
    messageType?: FavoriteMessageType,
  ): Promise<boolean> {
    const inferredType = messageType ?? inferMessageType(content);
    try {
      const isFavorited = await favoriteApi.toggleFavorite(
        messageId,
        chatId,
        content,
        role,
        inferredType,
      );
      this.state.isFavoritedMap[messageId] = isFavorited;
      if (isFavorited) {
        await this.loadFavorites();
      } else {
        this.state.favorites = this.state.favorites.filter(
          (f) => f.messageId !== messageId,
        );
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
        map[f.messageId] = true;
      }
      this.state.isFavoritedMap = map;
    } catch (error) {
      console.error("Failed to load favorites:", error);
    } finally {
      this.state.isLoading = false;
    }
  }

  async checkFavorited(messageId: UUID): Promise<boolean> {
    try {
      const isFavorited = await favoriteApi.isFavorited(messageId);
      this.state.isFavoritedMap[messageId] = isFavorited;
      return isFavorited;
    } catch (error) {
      console.error("Failed to check favorite:", error);
      return false;
    }
  }

  async addTag(favoriteId: UUID, tag: string): Promise<void> {
    try {
      await favoriteApi.addTag(favoriteId, tag);
      await this.loadFavorites();
    } catch (error) {
      console.error("Failed to add tag:", error);
      throw error;
    }
  }

  async removeTag(favoriteId: UUID, tag: string): Promise<void> {
    try {
      await favoriteApi.removeTag(favoriteId, tag);
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
  }
}

function inferMessageType(content: string): FavoriteMessageType {
  if (content.length >= 500 || content.includes('```')) {
    return 'chat';
  }

  if (content.length < 200 && !content.includes('```')) {
    return 'text';
  }

  return 'message';
}

export const favoriteStore = new FavoriteStore();
