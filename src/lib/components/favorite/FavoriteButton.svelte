<script lang="ts">
  import { Star } from "lucide-svelte";
  import { t } from "$lib/i18n";
  import { favoriteStore } from "$lib/states";
  import type { FavoriteMessageType } from "$lib/types/favorite";
  import type { UUID } from "$lib/types";

  interface Props {
    messageId: UUID;
    chatId: UUID;
    content: string;
    role: 'user' | 'assistant' | 'system';
    messageType?: FavoriteMessageType;
    size?: "sm" | "md" | "lg";
  }

  let {
    messageId,
    chatId,
    content,
    role,
    messageType,
    size = "md",
  }: Props = $props();

  const sizeClasses = {
    sm: "w-3.5 h-3.5",
    md: "w-4 h-4",
    lg: "w-5 h-5",
  };

  let isFavorited = $derived(favoriteStore.isFavorited(messageId, chatId, messageType ?? 'message'));
  let isLoading = $state(false);

  async function handleToggle() {
    if (isLoading) return;
    isLoading = true;
    try {
      await favoriteStore.toggleFavorite(messageId, chatId, content, role, messageType);
    } catch (error) {
      console.error("Failed to toggle favorite:", error);
    } finally {
      isLoading = false;
    }
  }
</script>

<button
  class="text-base-content/60 hover:text-amber-500 hover:bg-amber-500/10 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
  title={isFavorited ? t("favorites.toggle.unfavorite") : t("favorites.toggle.favorite")}
  onclick={handleToggle}
  disabled={isLoading}
>
  {#if isLoading}
    <div class="{sizeClasses[size]} border-2 border-current border-t-transparent rounded-full animate-spin"></div>
  {:else}
    <Star class="{sizeClasses[size]} {isFavorited ? 'fill-amber-500 text-amber-500' : ''}" />
  {/if}
</button>
