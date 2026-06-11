<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { browser } from "$app/environment";
  import {
    Search,
    Filter,
    X,
    ExternalLink,
    Tag,
    Trash2,
    MoreVertical,
    Pencil,
    Star,
    ChevronDown,
    ChevronUp,
  } from "@lucide/svelte";
  import { favoriteStore } from "$lib/states";
  import type {
    Favorite,
    FavoriteMessageType,
    FavoriteTag,
    TagColor,
    TextRange,
  } from "$lib/types/favorite";
  import { highlightRange, renderMarkdown } from "$lib/utils";
  import { escapeHtml } from "$lib/utils/string";
  import { resolveLocalAssetPath } from "$lib/utils/tauri";
  import { openInBrowser } from "$lib/utils/browser";

  let searchQuery = $state("");
  let selectedType = $state<FavoriteMessageType | "all">("all");
  let selectedTags = $state<string[]>([]);
  let expandedFavorites = $state<Record<string, boolean>>({});
  let tagFilterExpanded = $state(false);
  let tagFilterOverflow = $state(false);
  let tagFilterEl = $state<HTMLDivElement | null>(null);

  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let selectedFavorite = $state<Favorite | null>(null);

  let showTagEditor = $state(false);
  let editorX = $state(0);
  let editorY = $state(0);
  let editingFavoriteId = $state<string | null>(null);
  let newTagName = $state("");
  let newTagColor = $state<TagColor>("info");
  let tagFilterResizeObserver: ResizeObserver | null = null;
  let observedTagFilterEl: HTMLDivElement | null = null;

  const COLLAPSED_TAG_FILTER_HEIGHT = 48;

  const messageTypes: { value: FavoriteMessageType | "all"; label: string }[] =
    [
      { value: "all", label: "全部" },
      { value: "text", label: "文本" },
      { value: "image", label: "图片" },
      { value: "message", label: "消息" },
      { value: "chat", label: "对话" },
      { value: "external", label: "外部" },
    ];

  const tagColors: { value: TagColor; label: string; class: string }[] = [
    {
      value: "primary",
      label: "主色",
      class: "bg-primary text-primary-content",
    },
    {
      value: "secondary",
      label: "次要",
      class: "bg-secondary text-secondary-content",
    },
    { value: "accent", label: "强调", class: "bg-accent text-accent-content" },
    {
      value: "success",
      label: "成功",
      class: "bg-success text-success-content",
    },
    {
      value: "warning",
      label: "警告",
      class: "bg-warning text-warning-content",
    },
    { value: "error", label: "错误", class: "bg-error text-error-content" },
    { value: "info", label: "信息", class: "bg-info text-info-content" },
    { value: "gray", label: "灰色", class: "bg-base-300 text-base-content" },
  ];

  let filteredFavorites = $derived.by(() => {
    let result = favoriteStore.favorites;

    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      result = result.filter((f) => {
        const content = getSearchableText(f);
        return (
          content.toLowerCase().includes(query) ||
          f.tags.some((t) => t.name.toLowerCase().includes(query))
        );
      });
    }

    if (selectedType !== "all") {
      result = result.filter((f) => f.messageType === selectedType);
    }

    if (selectedTags.length > 0) {
      result = result.filter((f) =>
        selectedTags.some((tag) => f.tags.some((t) => t.name === tag))
      );
    }

    return result;
  });

  let allTags = $derived.by(() => {
    const tags = new Set<string>();
    for (const tag of favoriteStore.tags) {
      tags.add(tag.name);
    }
    return Array.from(tags).sort();
  });

  function getDisplayContent(favorite: Favorite): string {
    switch (favorite.messageType) {
      case "chat":
        return favorite.content;
      case "text":
        return favorite.context ?? favorite.content;
      case "external":
        return favorite.content;
      case "image":
        const match = favorite.content.match(/!\[.*?\]\((.*?)\)/);
        return match ? match[1] : favorite.content;
      case "message":
      default:
        return favorite.content;
    }
  }

  function getSearchableText(favorite: Favorite): string {
    const base = getDisplayContent(favorite);
    const metadata = [
      favorite.sourceAppName,
      favorite.sourceBundleId,
      favorite.sourceWindowTitle,
      favorite.sourceUrl,
      favorite.sourceDomain,
      favorite.sourceTabTitle,
    ]
      .filter(Boolean)
      .join(" ");
    return [base, metadata].filter(Boolean).join(" ");
  }

  function parseTextRanges(content: string): TextRange[] {
    try {
      const parsed = JSON.parse(content) as TextRange[] | TextRange;
      if (Array.isArray(parsed)) return parsed;
      if (parsed && typeof parsed === "object") return [parsed];
      return [];
    } catch {
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
        current = {
          start: current.start,
          end: Math.max(current.end, next.end),
        };
      } else {
        merged.push(current);
        current = next;
      }
    }
    merged.push(current);
    return merged;
  }

  function getPlainTextFromMarkdown(markdown: string): string {
    if (!browser) return markdown;
    const html = renderMarkdown(markdown);
    const container = document.createElement("div");
    container.innerHTML = html;
    return container.textContent ?? "";
  }

  function isTextContentLong(content: string): boolean {
    if (!content) return false;
    const length = content.length;
    const paragraphCount = content.split(/\n{2,}/).filter(Boolean).length;
    const lineCount = content.split(/\n/).length;
    return length > 200 || paragraphCount > 2 || lineCount > 6;
  }

  function buildRangeSummary(text: string, ranges: TextRange[]): string {
    if (!browser) return text;
    const normalized = mergeTextRanges(ranges);
    if (normalized.length === 0) return escapeHtml(text);

    const pieces: string[] = [];
    normalized.forEach((range, index) => {
      const snippet = escapeHtml(text.slice(range.start, range.end));
      pieces.push(
        `<span class="text-base-content/50 text-xs">段落${index + 1}</span>`
      );
      pieces.push("<br />");
      pieces.push(`<span class="px-1 rounded">${snippet}</span>`);
      if (index < normalized.length - 1) {
        pieces.push("<br />");
      }
    });

    return pieces.join("");
  }

  function getImageSrc(content: string): string | null {
    const match = content.match(/!\[.*?\]\((.*?)\)/);
    if (!match) return null;
    return resolveLocalAssetPath(match[1]);
  }

  function toggleExpand(favoriteId?: string) {
    if (!favoriteId) return;
    expandedFavorites[favoriteId] = !expandedFavorites[favoriteId];
  }

  function isExpanded(favoriteId: string | undefined): boolean {
    if (!favoriteId) return false;
    return expandedFavorites[favoriteId] ?? false;
  }

  function shouldShowExpandButton(favorite: Favorite): boolean {
    if (!favorite.id) return false;
    if (favorite.messageType === "text" && favorite.context) {
      return isTextContentLong(favorite.context);
    }
    const content = getDisplayContent(favorite);
    const lines = content.split("\n");
    return lines.length > 3 || content.length > 300;
  }

  function handleContextMenu(event: MouseEvent, favorite: Favorite) {
    event.preventDefault();
    event.stopPropagation();

    selectedFavorite = favorite;
    const position = clampOverlayPosition(event.clientX, event.clientY, 180, 110);
    contextMenuX = position.x;
    contextMenuY = position.y;
    showContextMenu = true;
  }

  function closeContextMenu() {
    showContextMenu = false;
    selectedFavorite = null;
  }

  function handleEditTags(favorite: Favorite) {
    closeContextMenu();
    editingFavoriteId = favorite.id ?? null;
    const position = clampOverlayPosition(contextMenuX, contextMenuY, 320, 360);
    editorX = position.x;
    editorY = position.y;
    showTagEditor = true;
    newTagName = "";
    newTagColor = "info";
  }

  async function handleAddTag() {
    if (!editingFavoriteId || !newTagName.trim()) return;

    try {
      await favoriteStore.addTag(
        editingFavoriteId,
        newTagName.trim(),
        newTagColor
      );
      newTagName = "";
      newTagColor = "info";
    } catch (error) {
      console.error("Failed to add tag:", error);
    }
  }

  async function handleRemoveTag(tag: FavoriteTag) {
    if (!editingFavoriteId) return;
    try {
      await favoriteStore.removeTag(editingFavoriteId, tag.name);
    } catch (error) {
      console.error("Failed to remove tag:", error);
    }
  }

  async function handlePickTag(tag: FavoriteTag) {
    if (!editingFavoriteId) return;
    try {
      await favoriteStore.addTag(editingFavoriteId, tag.name, tag.color);
    } catch (error) {
      console.error("Failed to add tag:", error);
    }
  }

  async function handleDeleteFavorite(favorite: Favorite) {
    closeContextMenu();
    try {
      if (favorite.messageType === "text") {
        await favoriteStore.saveTextRanges(
          favorite.messageId,
          favorite.chatId,
          [],
          favorite.role,
          favorite.context
        );
      } else if (favorite.messageType === "external") {
        if (favorite.id) {
          await favoriteStore.deleteFavoriteById(favorite.id);
        }
      } else {
        await favoriteStore.toggleFavorite(
          favorite.messageId,
          favorite.chatId,
          favorite.content,
          favorite.role,
          favorite.messageType
        );
      }
    } catch (error) {
      console.error("Failed to remove favorite:", error);
    }
  }

  function handleNavigate(favorite: Favorite) {
    if (favorite.messageType === "external") {
      if (favorite.sourceUrl) {
        openInBrowser(favorite.sourceUrl).catch((error) =>
          console.error("Failed to open source URL:", error)
        );
      }
      return;
    }

    if (favorite.messageType === "chat") {
      goto(`/chat?id=${favorite.chatId}`);
      return;
    }

    goto(`/chat?id=${favorite.chatId}#message-${favorite.messageId}`);
  }

  function toggleTag(tag: string) {
    if (selectedTags.includes(tag)) {
      selectedTags = selectedTags.filter((t) => t !== tag);
    } else {
      selectedTags = [...selectedTags, tag];
    }
  }

  function updateTagFilterOverflow() {
    if (!tagFilterEl) {
      tagFilterOverflow = false;
      return;
    }

    tagFilterOverflow =
      tagFilterEl.scrollHeight > COLLAPSED_TAG_FILTER_HEIGHT + 1;
  }

  function syncTagFilterObserver() {
    if (!tagFilterResizeObserver || observedTagFilterEl === tagFilterEl) {
      return;
    }

    if (observedTagFilterEl) {
      tagFilterResizeObserver.unobserve(observedTagFilterEl);
    }

    observedTagFilterEl = tagFilterEl;

    if (observedTagFilterEl) {
      tagFilterResizeObserver.observe(observedTagFilterEl);
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

  function getMessageTypeLabel(type: FavoriteMessageType): string {
    const labels: Record<FavoriteMessageType, string> = {
      text: "文本",
      image: "图片",
      message: "消息",
      chat: "对话",
      external: "外部",
    };
    return labels[type] || type;
  }

  function getTagColorClass(color: TagColor): string {
    const colorMap: Record<TagColor, string> = {
      primary: "bg-primary/20 text-primary",
      secondary: "bg-secondary/20 text-secondary",
      accent: "bg-accent/20 text-accent",
      success: "bg-success/20 text-success",
      warning: "bg-warning/20 text-warning",
      error: "bg-error/20 text-error",
      info: "bg-info/20 text-info",
      gray: "bg-base-300 text-base-content/70",
    };
    return colorMap[color] || colorMap.info;
  }

  function handleClickOutside(event: MouseEvent) {
    if (!showContextMenu && !showTagEditor) return;
    const target = event.target as HTMLElement;
    if (!target.closest(".context-menu") && !target.closest(".tag-editor")) {
      closeOverlays();
    }
  }

  function handleOverlayScroll() {
    if (!showContextMenu && !showTagEditor) return;
    closeOverlays();
  }

  function closeOverlays() {
    closeContextMenu();
    showTagEditor = false;
    editingFavoriteId = null;
  }

  function clampOverlayPosition(x: number, y: number, width: number, height: number) {
    if (!browser) return { x, y };
    const padding = 12;
    const maxX = Math.max(padding, window.innerWidth - width - padding);
    const maxY = Math.max(padding, window.innerHeight - height - padding);
    return {
      x: Math.min(Math.max(x, padding), maxX),
      y: Math.min(Math.max(y, padding), maxY),
    };
  }

  function getNavigateLabel(favorite: Favorite): string | null {
    if (favorite.messageType === "external") {
      return favorite.sourceUrl ? "打开来源" : null;
    }
    return favorite.messageType === "chat" ? "查看对话" : "查看消息";
  }

  onMount(() => {
    if (browser) {
      tagFilterResizeObserver = new ResizeObserver(updateTagFilterOverflow);
      syncTagFilterObserver();
      favoriteStore.loadFavorites();
      favoriteStore.loadTags();
    }

    return () => {
      tagFilterResizeObserver?.disconnect();
      tagFilterResizeObserver = null;
      observedTagFilterEl = null;
    };
  });

  $effect(() => {
    allTags.length;
    selectedTags.length;
    tagFilterExpanded;
    tagFilterEl;

    if (browser) {
      syncTagFilterObserver();
      tick().then(updateTagFilterOverflow);
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
      <div class="mt-3">
        <div
          bind:this={tagFilterEl}
          class="flex flex-wrap items-center gap-1.5 overflow-hidden transition-[max-height] duration-200"
          style:max-height={tagFilterExpanded
            ? "none"
            : `${COLLAPSED_TAG_FILTER_HEIGHT}px`}
        >
          {#each allTags as tag}
            <button
              class="inline-flex h-5 max-w-36 items-center rounded-full border px-1.5 text-[11px] leading-none transition-colors cursor-pointer
                {selectedTags.includes(tag)
                ? 'bg-primary text-primary-content border-primary'
                : 'bg-base-200 text-base-content border-base-300 hover:border-primary/50'}"
              title={tag}
              onclick={() => toggleTag(tag)}
            >
              <span class="truncate">{tag}</span>
            </button>
          {/each}
          {#if selectedTags.length > 0}
            <button
              class="inline-flex h-5 items-center rounded-full border border-dashed border-base-300 px-1.5 text-[11px] leading-none text-base-content/50 hover:text-base-content hover:border-base-content/50 transition-colors cursor-pointer"
              onclick={() => (selectedTags = [])}
            >
              清除
            </button>
          {/if}
        </div>
        {#if tagFilterOverflow}
          <button
            class="mt-1.5 inline-flex h-6 items-center gap-1 rounded-md px-1.5 text-[11px] text-base-content/60 hover:bg-base-200 hover:text-base-content transition-colors cursor-pointer"
            onclick={() => (tagFilterExpanded = !tagFilterExpanded)}
          >
            {#if tagFilterExpanded}
              <ChevronUp size={12} />
              收起
            {:else}
              <ChevronDown size={12} />
              展开
            {/if}
          </button>
        {/if}
      </div>
    {/if}
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-4" onscroll={handleOverlayScroll}>
    {#if favoriteStore.isLoading}
      <div class="flex items-center justify-center h-full">
        <div
          class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
        ></div>
      </div>
    {:else if filteredFavorites.length === 0}
      <div
        class="flex flex-col items-center justify-center h-full text-base-content/50"
      >
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
          {@const navLabel = getNavigateLabel(favorite)}
          <div
            class="bg-base-200 rounded-lg p-4 hover:bg-base-300 transition-colors relative {selectedFavorite?.id ===
              favorite.id && showContextMenu
              ? 'bg-base-300'
              : ''}"
            oncontextmenu={(e) => handleContextMenu(e, favorite)}
          >
            <div class="flex items-start justify-between mb-2 gap-3">
              <div class="flex flex-wrap items-center gap-2">
                <span class="px-2 py-0.5 text-xs rounded-full bg-info/20 text-info">
                  {getMessageTypeLabel(favorite.messageType)}
                </span>
                {#if favorite.tags.length > 0}
                  <div class="flex flex-wrap items-center gap-1">
                    {#each favorite.tags as tag}
                      <span
                        class="inline-flex h-5 max-w-32 items-center gap-1 rounded-full px-1.5 text-[11px] leading-none {getTagColorClass(
                          tag.color
                        )}"
                        title={tag.name}
                      >
                        <Tag size={10} />
                        <span class="truncate">{tag.name}</span>
                      </span>
                    {/each}
                  </div>
                {/if}
              </div>

              <div class="flex items-center gap-3 text-xs text-base-content/50 whitespace-nowrap">
                <span>{formatTime(favorite.createdAt)}</span>
                {#if navLabel}
                  <button
                    class="text-xs text-base-content/50 hover:text-primary flex items-center gap-1 cursor-pointer"
                    onclick={() => handleNavigate(favorite)}
                  >
                    <ExternalLink size={12} />
                    {navLabel}
                  </button>
                {/if}
              </div>
            </div>

            <div class="mb-2">
              {#if favorite.messageType === "chat"}
                <h3 class="font-medium text-base-content mb-1">
                  {favorite.content}
                </h3>
              {/if}

              <div class="text-sm text-base-content">
                {#if favorite.messageType === "image"}
                  {#if getImageSrc(favorite.content)}
                    <img
                      src={getImageSrc(favorite.content)}
                      alt="收藏的图片"
                      class="max-h-48 rounded-lg object-contain"
                    />
                  {:else}
                    <p class="text-sm text-base-content/70 italic">
                      {favorite.content}
                    </p>
                  {/if}
                {:else if favorite.messageType === "message"}
                  <div
                    class="break-words text-[15px] leading-[1.6] markdown-content {favorite.id &&
                    !isExpanded(favorite.id)
                      ? 'line-clamp-3'
                      : ''}"
                  >
                    {@html renderMarkdown(favorite.content || "")}
                  </div>
                  {#if favorite.id && shouldShowExpandButton(favorite)}
                    <button
                      class="text-xs text-primary hover:underline mt-2 cursor-pointer flex items-center gap-1"
                      onclick={() => toggleExpand(favorite.id)}
                    >
                      {#if isExpanded(favorite.id)}
                        收起
                      {:else}
                        展开消息
                      {/if}
                    </button>
                  {/if}
                {:else if favorite.messageType === "external"}
                  <div class="space-y-2">
                    <div class="whitespace-pre-wrap text-[15px] leading-[1.6]">
                      {favorite.content}
                    </div>
                    {#if favorite.selectionTextRaw &&
                    favorite.selectionTextRaw !== favorite.content}
                      <div class="text-xs text-base-content/60">
                        原文: {favorite.selectionTextRaw}
                      </div>
                    {/if}
                    <div class="flex flex-wrap gap-2 text-xs text-base-content/60">
                      {#if favorite.sourceAppName}
                        <span>应用: {favorite.sourceAppName}</span>
                      {/if}
                      {#if favorite.sourceWindowTitle}
                        <span>窗口: {favorite.sourceWindowTitle}</span>
                      {/if}
                      {#if favorite.sourceUrl}
                        <span>链接: {favorite.sourceUrl}</span>
                      {/if}
                      {#if favorite.sourceDomain}
                        <span>域名: {favorite.sourceDomain}</span>
                      {/if}
                      {#if favorite.sourceTabTitle}
                        <span>标签: {favorite.sourceTabTitle}</span>
                      {/if}
                    </div>
                  </div>
                {:else if favorite.messageType === "text"}
                  {@const ranges = mergeTextRanges(
                    parseTextRanges(favorite.content)
                  )}
                  {#if favorite.context && ranges.length > 0}
                    {@const isLong = isTextContentLong(favorite.context)}
                    {#if isLong && favorite.id && !isExpanded(favorite.id)}
                      {@const plainText = getPlainTextFromMarkdown(
                        favorite.context
                      )}
                      <div
                        class="whitespace-pre-wrap text-[15px] leading-[1.6]"
                      >
                        {@html buildRangeSummary(plainText, ranges)}
                      </div>
                      <button
                        class="text-xs text-primary hover:underline mt-2 cursor-pointer flex items-center gap-1"
                        onclick={() => toggleExpand(favorite.id)}
                      >
                        展开消息
                      </button>
                    {:else}
                      <div
                        class="break-words text-[15px] leading-[1.6] markdown-content {favorite.id &&
                        !isExpanded(favorite.id)
                          ? 'line-clamp-3'
                          : ''}"
                        use:highlightRange={ranges}
                      >
                        {@html renderMarkdown(favorite.context || "")}
                      </div>
                      {#if favorite.id && shouldShowExpandButton(favorite)}
                        <button
                          class="text-xs text-primary hover:underline mt-2 cursor-pointer flex items-center gap-1"
                          onclick={() => toggleExpand(favorite.id)}
                        >
                          {#if isExpanded(favorite.id)}
                            收起
                          {:else}
                            展开消息
                          {/if}
                        </button>
                      {/if}
                    {/if}
                  {:else if favorite.context}
                    <p class="text-sm text-base-content/70 italic">
                      无效的文本范围
                    </p>
                  {:else}
                    <p class="text-sm text-base-content/70 italic">
                      数据格式已更新，请重新收藏
                    </p>
                  {/if}
                {/if}
              </div>
            </div>

          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- 右键菜单 -->
{#if showContextMenu && selectedFavorite}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
    onclick={(event) => event.stopPropagation()}
  >
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={() => handleEditTags(selectedFavorite!)}
    >
      <Pencil size={14} />
      编辑标签
    </button>
    <div class="border-t border-base-300 my-1 mx-2"></div>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
      onclick={() => handleDeleteFavorite(selectedFavorite!)}
    >
      <Trash2 size={14} />
      删除收藏
    </button>
  </div>
{/if}

<!-- 标签编辑弹窗 -->
{#if showTagEditor && editingFavoriteId}
  <div
    class="tag-editor fixed z-[10030] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl p-4 min-w-[280px]"
    style="left: {editorX}px; top: {editorY}px;"
    onclick={(event) => event.stopPropagation()}
  >
    <div class="flex items-center gap-2 mb-3">
      <Tag size={16} />
      <h3 class="text-sm font-medium">编辑标签</h3>
    </div>

    {#if editingFavoriteId}
      {@const currentTags =
        favoriteStore.favorites.find((f) => f.id === editingFavoriteId)?.tags ??
        []}
      {@const currentTagNames = new Set(currentTags.map((tag) => tag.name))}

      <div class="flex flex-wrap gap-1 mb-3">
        {#each currentTags as tag}
          <span
            class="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full {getTagColorClass(
              tag.color
            )}"
          >
            {tag.name}
            <button
              class="hover:text-error ml-1"
              onclick={() => handleRemoveTag(tag)}
            >
              <X size={10} />
            </button>
          </span>
        {/each}
      </div>

      {#if favoriteStore.tags.length > 0}
        <div class="mb-3">
          <p class="text-xs text-base-content/60 mb-1">已有标签</p>
          <div class="flex flex-wrap gap-1">
            {#each favoriteStore.tags.filter((tag) => !currentTagNames.has(tag.name)) as tag}
              <button
                class="inline-flex items-center gap-1 px-2 py-0.5 text-xs rounded-full {getTagColorClass(
                  tag.color
                )} hover:opacity-80 transition-opacity"
                onclick={() => handlePickTag(tag)}
              >
                {tag.name}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    {/if}

    <div class="flex gap-2 mb-3">
      <input
        type="text"
        placeholder="标签名称..."
        class="flex-1 h-8 px-2 text-xs bg-base-200 rounded border border-base-300 focus:outline-none focus:border-primary"
        bind:value={newTagName}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            handleAddTag();
          }
        }}
      />
      <button
        class="h-8 px-3 text-xs rounded bg-primary text-primary-content hover:bg-primary/90"
        onclick={handleAddTag}
      >
        添加
      </button>
    </div>

    <div class="flex flex-wrap gap-1">
      {#each tagColors as color}
        <button
          class={`w-6 h-6 rounded-full ${color.class} border-2 border-transparent hover:border-base-content/30 transition-colors ${newTagColor === color.value ? "ring-2 ring-offset-2 ring-offset-base-100 ring-base-content/40" : ""}`}
          title={color.label}
          onclick={() => (newTagColor = color.value)}
        ></button>
      {/each}
    </div>
  </div>
{/if}

<svelte:window onclick={handleClickOutside} onresize={handleOverlayScroll} />
