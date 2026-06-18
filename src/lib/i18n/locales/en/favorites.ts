/**
 * favorites namespace strings (filled by migration subagent).
 */
import type { favoritesZh } from "../zh/favorites";

export const favoritesEn: Record<keyof typeof favoritesZh, string> = {
  "favorites.title": "Favorites",
  "favorites.count": "{n} items",
  // favorite button
  "favorites.toggle.favorite": "Favorite",
  "favorites.toggle.unfavorite": "Unfavorite",
  // search and filter
  "favorites.searchPlaceholder": "Search favorite content or tags...",
  "favorites.clearFilter": "Clear filters",
  // message types
  "favorites.type.text": "Text",
  "favorites.type.image": "Image",
  "favorites.type.message": "Message",
  "favorites.type.chat": "Chat",
  "favorites.type.external": "External",
  // tag colors
  "favorites.color.primary": "Primary",
  "favorites.color.secondary": "Secondary",
  "favorites.color.accent": "Accent",
  "favorites.color.success": "Success",
  "favorites.color.warning": "Warning",
  "favorites.color.error": "Error",
  "favorites.color.info": "Info",
  "favorites.color.gray": "Gray",
  // roles
  "favorites.role.user": "User",
  "favorites.role.assistant": "Assistant",
  "favorites.role.system": "System",
  // empty states
  "favorites.empty.noMatch": "No matching favorites found",
  "favorites.empty.none": "No favorited messages yet",
  "favorites.empty.hint": "Click the star icon next to a message to favorite it",
  // list items
  "favorites.imageAlt": "Favorited image",
  "favorites.expandMessage": "Expand message",
  "favorites.collapse": "Collapse",
  "favorites.openSource": "Open source",
  "favorites.viewChat": "View chat",
  "favorites.viewMessage": "View message",
  // external source fields
  "favorites.source.original": "Original",
  "favorites.source.app": "App",
  "favorites.source.window": "Window",
  "favorites.source.url": "URL",
  "favorites.source.domain": "Domain",
  "favorites.source.tab": "Tab",
  // text ranges
  "favorites.paragraph": "Paragraph {n}",
  "favorites.invalidRange": "Invalid text range",
  "favorites.outdatedFormat": "Data format updated, please favorite again",
  // context menu
  "favorites.editTags": "Edit tags",
  "favorites.deleteFavorite": "Delete favorite",
  // tag editor
  "favorites.existingTags": "Existing tags",
  "favorites.tagNamePlaceholder": "Tag name...",
};
