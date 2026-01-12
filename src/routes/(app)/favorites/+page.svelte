<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { browser } from "$app/environment";
  import { Search, Filter, X, Star, Copy, ExternalLink, Tag, Trash2 } from "lucide/svelte";
  import { favoriteStore } from "$lib/states";
  import type { Favorite, FavoriteMessageType } from "$lib/types/favorite";

  let searchQuery = $state("");
  let selectedType = $state<FavoriteMessageType | "all">("all");
  let selectedTags = $state<string[]>([]);
  let newTagInput = $state("");

  const messageTypes: { value: FavoriteMessageType | "all"; label: string }[] = [
    { value: "all", label: "全部" },
    { value: "text", label: "文本" },
    { value: "image", label: "图片" },
    { value: "message", label: "消息" },
    { value: "chat", label: "对话" },
    { value: "other", label: "其它" },
  ];

  let filteredFavorites = $derived.by(() => {
    let result = favoriteStore.favorites;

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      result = result.filter(
        (f) =>
          f.content.toLowerCase().includes(query) ||
          f.tags.some((t) => t.toLowerCase().includes(query)),
      );
    }

    if (selectedType !== "all") {
      result = result.filter((f) => f.messageType === selectedType);
    }

    if (selectedTags.length > 0) {
      result = result.filter((f) =>
        selectedTags.some((tag) => f.tags.includes(tag)),
      );
    }

    return result;
  });

  let allTags = $derived.by(() => {
    const tags = new Set<string>();
    for (const f of favoriteStore.favorites) {
      for (const tag of f.tags) {
        tags.add(tag);
      }
    }
    return Array.from(tags).sort();
  });

  async function handleCopyContent(content: string) {
    try {
      await navigator.clipboard.writeText(content);
    } catch (error) {
      console.error("Failed to copy:", error);
    }
  }

  function handleNavigateToChat(chatId: string) {
    goto(`/chat?id=${chatId}`);
  }

  async function handleRemoveFavorite(favorite: Favorite) {
    try {
      await favoriteStore.toggleFavorite(
        favorite.messageId,
        favorite.chatId,
        favorite.content,
        favorite.role,
        favorite.messageType,
      );
    } catch (error) {
      console.error("Failed to remove favorite:", error);
    }
  }

  async function handleAddTag(favoriteId: string, tag: string) {
    if (!tag.trim()) return;
    try {
      await favoriteStore.addTag(favoriteId, tag);
      newTagInput = "";
    } catch (error) {
      console.error("Failed to add tag:", error);
    }
  }

  async function handleRemoveTag(favoriteId: string, tag: string) {
    try {
      await favoriteStore.removeTag(favoriteId, tag);
    } catch (error) {
      console.error("Failed to remove tag:", error);
    }
  }

  function toggleTag(tag: string) {
    if (selectedTags.includes(tag)) {
      selectedTags = selectedTags.filter((t) => t !== tag);
    } else {
      selectedTags = [...selectedTags, tag];
    }
  }

  function formatTime(timestamp: number): string {
    return new Date(timestamp).toLocaleString("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function getRoleLabel(role: string): string {
    switch (role) {
      case "user":
        return "用户";
      case "assistant":
        return "助手";
      case "system":
        return "系统";
      default:
        return role;
    }
  }

  onMount(() => {
    if (browser) {
      favoriteStore.loadFavorites();
    }
  });
</script>

<div class="h-full flex flex-col">
  <div class="flex-shrink-0 p-4 border-b border-base-300">
    <div class="flex items-center gap-4 mb-4">
      <h1 class="text-xl font-semibold text-base-content">收藏</h1>
      <span class="text-sm text-base-content/60">
        {filteredFavorites.length} 条
      </span>
    </div>

    <div class="flex flex-wrap gap-3">
      <div class="relative flex-1 min-w-[200px]">
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
          size={16}
        />
        <input
          type="text"
          placeholder="搜索收藏内容或标签..."
          class="w-full h-9 pl-10 pr-4 bg-base-200 rounded-lg text-base-content placeholder:text-base-content/50 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
          bind:value={searchQuery}
        />
        {#if searchQuery}
          <button
            class="absolute right-3 top-1/2 -translate-y-1/2 text-base-content/50 hover:text-base-content"
            onclick={() => (searchQuery = "")}
          >
            <X size={14} />
          </button>
        {/if}
      </div>

      <div class="relative">
        <select
          class="h-9 px-3 bg-base-200 rounded-lg text-base-content text-sm focus:outline-none focus:ring-2 focus:ring-primary/50 appearance-none cursor-pointer pr-8"
          bind:value={selectedType}
        >
          {#each messageTypes as type}
            <option value={type.value}>{type.label}</option>
          {/each}
        </select>
        <Filter
          class="absolute right-2 top-1/2 -translate-y-1/2 text-base-content/50 pointer-events-none"
          size={14}
        />
      </div>
    </div>

    {#if allTags.length > 0}
      <div class="flex flex-wrap gap-2 mt-3">
        {#each allTags as tag}
          <button
            class="px-2 py-1 text-xs rounded-full border transition-colors cursor-pointer
              {selectedTags.includes(tag)
                ? 'bg-primary text-primary-content border-primary'
                : 'bg-base-200 text-base-content border-base-300 hover:border-primary/50'}"
            onclick={() => toggleTag(tag)}
          >
            {tag}
          </button>
        {/each}
        {#if selectedTags.length > 0}
          <button
            class="px-2 py-1 text-xs rounded-full border border-dashed border-base-300 text-base-content/50 hover:text-base-content hover:border-base-content/50 transition-colors cursor-pointer"
            onclick={() => (selectedTags = [])}
          >
            清除筛选
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-4">
    {#if favoriteStore.isLoading}
      <div class="flex items-center justify-center h-full">
        <div
          class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
        ></div>
      </div>
    {:else if filteredFavorites.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-base-content/50">
        <Star size={48} class="mb-4 opacity-20" />
        {#if searchQuery || selectedType !== "all" || selectedTags.length > 0}
          <p class="mb-2">没有找到匹配的收藏</p>
          <button
            class="text-primary hover:underline cursor-pointer"
            onclick={() => {
              searchQuery = "";
              selectedType = "all";
              selectedTags = [];
            }}
          >
            清除筛选
          </button>
        {:else}
          <p>还没有收藏任何消息</p>
          <p class="text-sm mt-2">点击消息旁的星号图标即可收藏</p>
        {/if}
      </div>
    {:else}
      <div class="space-y-3">
        {#each filteredFavorites as favorite (favorite.id)}
          <div
            class="bg-base-200 rounded-xl p-4 hover:bg-base-300 transition-colors"
          >
            <div class="flex items-start justify-between gap-4">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 mb-2">
                  <span
                    class="px-2 py-0.5 text-xs rounded-full
                      {favorite.role === 'user'
                        ? 'bg-primary/20 text-primary'
                        : favorite.role === 'assistant'
                          ? 'bg-success/20 text-success'
                          : 'bg-base-300 text-base-content/60'}"
                  >
                    {getRoleLabel(favorite.role)}
                  </span>
                  <span class="text-xs text-base-content/50">
                    {formatTime(favorite.createdAt)}
                  </span>
                  <button
                    class="text-xs text-base-content/50 hover:text-primary flex items-center gap-1 cursor-pointer"
                    onclick={() => handleNavigateToChat(favorite.chatId)}
                  >
                    <ExternalLink size={12} />
                    查看对话
                  </button>
                </div>

                <p class="text-sm text-base-content line-clamp-3 whitespace-pre-wrap">
                  {favorite.content}
                </p>

                {#if favorite.tags.length > 0}
                  <div class="flex flex-wrap gap-1 mt-2">
                    {#each favorite.tags as tag}
                      <span
                        class="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full bg-base-100 text-base-content/70"
                      >
                        <Tag size={10} />
                        {tag}
                        <button
                          class="hover:text-error cursor-pointer"
                          onclick={() => handleRemoveTag(favorite.id!, tag)}
                        >
                          <X size={10} />
                        </button>
                      </span>
                    {/each}
                  </div>
                {/if}

                <div class="flex items-center gap-2 mt-3">
                  <input
                    type="text"
                    placeholder="添加标签..."
                    class="h-7 px-2 text-xs bg-base-100 rounded border border-base-300 focus:outline-none focus:border-primary"
                    bind:value={newTagInput}
                    onkeydown={(e) => {
                      if (e.key === "Enter") {
                        handleAddTag(favorite.id!, newTagInput);
                      }
                    }}
                  />
                </div>
              </div>

              <div class="flex flex-col gap-1">
                <button
                  class="p-1.5 text-base-content/60 hover:text-base-content hover:bg-base-100 rounded transition-colors cursor-pointer"
                  title="复制内容"
                  onclick={() => handleCopyContent(favorite.content)}
                >
                  <Copy size={14} />
                </button>
                <button
                  class="p-1.5 text-base-content/60 hover:text-error hover:bg-error/10 rounded transition-colors cursor-pointer"
                  title="取消收藏"
                  onclick={() => handleRemoveFavorite(favorite)}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
