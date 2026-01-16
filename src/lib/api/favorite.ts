import { apiCall } from "./index";
import type { Favorite, FavoriteMessageType, FavoriteTag } from "$lib/types/favorite";
import type { UUID } from "$lib/types";
import type { TextRange } from "$lib/types/favorite";

export async function toggleFavorite(
  messageId: UUID,
  chatId: UUID,
  content: string,
  role: 'user' | 'assistant' | 'system',
  messageType: FavoriteMessageType,
  tags: FavoriteTag[] = [],
  note?: string,
  context?: string,
): Promise<boolean> {
  return apiCall<boolean>("favorite_toggle", {
    request: {
      messageId,
      chatId,
      content,
      role,
      messageType,
      tags,
      note,
      context,
    },
  });
}

export async function isFavorited(messageId: UUID, chatId: UUID, messageType: FavoriteMessageType): Promise<boolean> {
  return apiCall<boolean>("favorite_is_favorited", {
    request: { messageId, chatId, messageType },
  });
}

export async function getFavorites(): Promise<Favorite[]> {
  return apiCall<Favorite[]>("favorite_list", {});
}

export async function getFavoritesByChat(chatId: UUID): Promise<Favorite[]> {
  return apiCall<Favorite[]>("favorite_list_by_chat", {
    request: { chatId },
  });
}

export async function listTags(): Promise<FavoriteTag[]> {
  return apiCall<FavoriteTag[]>("favorite_list_tags", {});
}

export async function saveTextRanges(
  messageId: UUID,
  chatId: UUID,
  ranges: TextRange[],
  role: 'user' | 'assistant' | 'system',
  context?: string,
): Promise<void> {
  return apiCall<void>("favorite_save_text_ranges", {
    request: {
      messageId,
      chatId,
      ranges,
      role,
      context,
    },
  });
}

export async function addTag(favoriteId: UUID, tag: FavoriteTag): Promise<void> {
  return apiCall<void>("favorite_add_tag", {
    request: { favoriteId, tag },
  });
}

export async function removeTag(favoriteId: UUID, tagName: string): Promise<void> {
  return apiCall<void>("favorite_remove_tag", {
    request: { favoriteId, tagName },
  });
}
