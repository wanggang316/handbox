<script lang="ts">
  import { Check, Trash2 } from "@lucide/svelte";
  export interface LookupItem {
    term: string;
    translation?: string | null;
    phonetic?: string | null;
    explanation?: string | null;
    sourceLanguage?: string | null;
    targetLanguage?: string | null;
    exists?: boolean;
  }

  interface Props {
    item: LookupItem;
    busy?: boolean;
    onAdd?: () => void;
    onDelete?: () => void;
    showDelete?: boolean;
  }

  let {
    item,
    busy = false,
    onAdd = () => {},
    onDelete = () => {},
    showDelete = false,
  }: Props = $props();
</script>

<div class="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
  <div>
    <div class="text-base font-medium flex items-center gap-2">
      <span>{item.term}</span>
      {#if item.phonetic}
        <span class="text-xs text-base-content/50">{item.phonetic}</span>
      {/if}
    </div>
    {#if item.translation}
      <div class="text-sm text-base-content/60">{item.translation}</div>
    {/if}
    {#if item.explanation}
      <div class="text-xs text-base-content/50 mt-1">{item.explanation}</div>
    {/if}
    {#if item.sourceLanguage}
      <div class="text-xs text-base-content/50 mt-1">
        {item.sourceLanguage}
      </div>
    {/if}
  </div>
  <div class="flex items-center gap-2">
    {#if item.exists}
      <div class="flex items-center gap-1 text-xs text-base-content/40">
        <Check size={12} />
        已在单词本
      </div>
    {:else}
      <button
        class="text-xs text-primary hover:text-primary/80"
        onclick={onAdd}
        disabled={busy || !item.translation}
      >
        添加到单词本
      </button>
    {/if}
    {#if showDelete}
      <button
        class="text-xs text-error/70 hover:text-error flex items-center gap-1"
        onclick={onDelete}
      >
        <Trash2 size={12} />
        删除
      </button>
    {/if}
  </div>
</div>
