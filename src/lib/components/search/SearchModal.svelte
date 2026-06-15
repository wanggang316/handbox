<script lang="ts">
  import Modal from '$lib/components/ui/Modal.svelte';
  import { Search as SearchIcon, Loader2, Clock, ArrowUpRight, X } from 'lucide-svelte';
  import { onMount, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import type { SearchResult } from '$lib/api/search';
  import { searchState } from '$lib/states';

  interface Props {
    open?: boolean;
    onClose?: () => void;
  }

  let {
    open = $bindable(false),
    onClose = () => {}
  }: Props = $props();

  let query = $state('');
  let hasLoadedHistory = $state(false);
  let searchInput: HTMLInputElement | undefined;

  const results = $derived(searchState.results);
  const history = $derived(searchState.history);
  const isLoading = $derived(searchState.isLoading);
  const error = $derived(searchState.error);

  onMount(() => {
    if (open) {
      initialiseState();
    }
  });

  $effect(() => {
    if (open) {
      initialiseState();
    }
  });

  async function initialiseState() {
    query = searchState.query ?? '';
    await tick();
    searchInput?.focus();

    if (!hasLoadedHistory) {
      try {
        await searchState.loadHistory();
      } catch (err) {
        console.error('Failed to load search history:', err);
      } finally {
        hasLoadedHistory = true;
      }
    }
  }

  async function performSearch(text: string) {
    const trimmed = text.trim();
    if (!trimmed) {
      return;
    }

    try {
      await searchState.search({
        query: trimmed,
        types: ['message'],
        limit: 20,
        sortBy: 'timestamp',
        sortOrder: 'desc'
      });
    } catch (err) {
      console.error('Search failed:', err);
    }
  }

  function handleClose() {
    open = false;
    onClose();
  }

  function handleSubmit(event: Event) {
    event.preventDefault();
    void performSearch(query);
  }

  function handleClear() {
    query = '';
  }

  async function handleHistorySelect(item: string) {
    query = item;
    await tick();
    void performSearch(item);
  }

  function splitSnippet(snippet: string, keyword: string) {
    if (!keyword.trim()) {
      return [{ text: snippet, highlight: false }];
    }

    const lowerSnippet = snippet.toLowerCase();
    const lowerKeyword = keyword.toLowerCase();
    const keywordLength = keyword.length;
    let cursor = 0;
    const parts: Array<{ text: string; highlight: boolean }> = [];

    while (cursor < snippet.length) {
      const index = lowerSnippet.indexOf(lowerKeyword, cursor);
      if (index === -1) {
        parts.push({ text: snippet.slice(cursor), highlight: false });
        break;
      }

      if (index > cursor) {
        parts.push({ text: snippet.slice(cursor, index), highlight: false });
      }

      parts.push({
        text: snippet.slice(index, index + keywordLength),
        highlight: true
      });

      cursor = index + keywordLength;
    }

    return parts;
  }

  async function handleResultSelect(result: SearchResult) {
    if (!result.chatId || !result.messageId) {
      return;
    }

    const focusKey = Date.now();
    handleClose();
    await goto(`/chat?id=${result.chatId}&message=${result.messageId}&focus=${focusKey}`);
  }

  function formatTimestamp(timestamp: number) {
    if (!timestamp) return '';
    const date = new Date(timestamp);
    return `${date.toLocaleDateString()} ${date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;
  }
</script>

<Modal
  bind:open
  onClose={handleClose}
  closeOnBackdropClick={true}
  showCloseButton={false}
>
  <div class="w-[640px] max-w-[90vw] h-[70vh] max-h-[70vh] flex flex-col">
    <form class="px-6 pt-6 pb-4" onsubmit={handleSubmit}>
      <div class="relative flex items-center">
        <SearchIcon class="absolute left-3 text-base-content/70" size={18} />
        <input
          bind:this={searchInput}
          type="text"
          class="w-full pl-10 pr-9 py-2.5 rounded-md border border-[var(--hairline)] bg-base-300 text-base-content focus:border-primary"
          placeholder="搜索聊天记录..."
          bind:value={query}
        />
        {#if query}
          <button
            type="button"
            class="absolute right-2 p-1 text-base-content/60 hover:text-base-content"
            aria-label="清空"
            onclick={handleClear}
          >
            <X size={16} />
          </button>
        {/if}
      </div>
    </form>

    <div class="flex-1 overflow-y-auto px-6 pb-6 space-y-4">
      {#if error}
        <div class="rounded-lg bg-error/10 text-error px-4 py-3 text-sm">
          搜索失败：{error}
        </div>
      {/if}

      {#if !query && history.length > 0}
        <div class="space-y-2">
          <div class="text-xs uppercase tracking-wide text-base-content/50 flex items-center gap-2">
            <Clock size={14} /> 历史搜索
          </div>
          <div class="flex flex-wrap gap-2">
            {#each history as item}
              <button
                type="button"
                class="px-3 py-1.5 rounded-full bg-base-300 text-sm text-base-content/80 hover:bg-base-300/80"
                onclick={() => handleHistorySelect(item)}
              >
                {item}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      {#if isLoading}
        <div class="flex items-center justify-center h-full text-base-content/70">
          <Loader2 class="mr-2 animate-spin" size={16} /> 正在搜索...
        </div>
      {:else if query && results.length === 0}
        <div class="text-center text-base-content/60 py-12">
          未找到与
          <span class="font-medium text-base-content">“{query}”</span>
          相关的聊天记录
        </div>
      {:else if results.length > 0}
        <div class="space-y-4">
          {#each results as result}
            <button
              type="button"
              class="w-full text-left rounded-lg border border-[var(--hairline)] hover:border-primary/40 hover:bg-primary/5 transition-colors px-4 py-3"
              onclick={() => handleResultSelect(result)}
            >
              <div class="flex items-start justify-between gap-4">
                <div class="flex-1 space-y-2">
                  <div class="text-sm font-medium text-base-content flex items-center gap-1">
                    {result.title}
                    <ArrowUpRight size={14} class="text-base-content/50" />
                  </div>
                  <div class="text-xs text-base-content/60">
                    {formatTimestamp(result.timestamp)}
                  </div>
                  <div class="text-sm text-base-content/80 leading-relaxed">
                    {#each splitSnippet(result.snippet, query) as part, index (index)}
                      {#if part.highlight}
                        <mark class="bg-primary/20 text-primary px-0.5 rounded">
                          {part.text}
                        </mark>
                      {:else}
                        <span>{part.text}</span>
                      {/if}
                    {/each}
                  </div>
                </div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</Modal>
