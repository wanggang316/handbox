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
): Promise<boolean> {
  return apiCall<boolean>("favorite_toggle", {
    message_id: messageId,
    chat_id: chatId,
    content,
    role,
    message_type: messageType,
    tags,
    note,
  });
}

export async function isFavorited(messageId: UUID): Promise<boolean> {
  return apiCall<boolean>("favorite_is_favorited", {
    message_id: messageId,
  });
}

export async function getFavorites(): Promise<Favorite[]> {
  return apiCall<Favorite[]>("favorite_list", {});
}

export async function getFavoritesByChat(chatId: UUID): Promise<Favorite[]> {
  return apiCall<Favorite[]>("favorite_list_by_chat", {
    chat_id: chatId,
  });
}

export async function addTag(favoriteId: UUID, tag: string): Promise<void> {
  return apiCall<void>("favorite_add_tag", {
    favorite_id: favoriteId,
    tag,
  });
}

export async function removeTag(favoriteId: UUID, tag: string): Promise<void> {
  return apiCall<void>("favorite_remove_tag", {
    favorite_id: favoriteId,
    tag,
  });
}
