import { apiCall } from "./index";
import type { Favorite, FavoriteMessageType } from "$lib/types/favorite";
import type { UUID } from "$lib/types";

export async function toggleFavorite(
  messageId: UUID,
  chatId: UUID,
  content: string,
  role: 'user' | 'assistant' | 'system',
  messageType: FavoriteMessageType,
  tags: string[] = [],
  note?: string,
  selectedText?: string,
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
      selectedText,
    },
  });
}

export async function isFavorited(messageId: UUID): Promise<boolean> {
  return apiCall<boolean>("favorite_is_favorited", {
    request: { messageId },
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

export async function addTag(favoriteId: UUID, tag: string): Promise<void> {
  return apiCall<void>("favorite_add_tag", {
    request: { favoriteId, tag },
  });
}

export async function removeTag(favoriteId: UUID, tag: string): Promise<void> {
  return apiCall<void>("favorite_remove_tag", {
    request: { favoriteId, tag },
  });
}
