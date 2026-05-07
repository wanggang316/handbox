<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeft } from "@lucide/svelte";
  import { getWord } from "$lib/api/word";
  import type { Word } from "$lib/types";

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let word = $state<Word | null>(null);

  const wordId = $derived($page.params.id);

  async function loadDetail() {
    if (!wordId) {
      errorMessage = "无效的单词 ID";
      return;
    }
    try {
      isLoading = true;
      errorMessage = null;
      word = await getWord(wordId);
    } catch (error) {
      console.error("Failed to load word detail:", error);
      errorMessage = "加载详情失败";
    } finally {
      isLoading = false;
    }
  }

  onMount(loadDetail);
</script>

<div class="h-full flex flex-col gap-4 p-6">
  <div class="pt-6">
    <button
      class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit"
      onclick={() => goto("/words")}
    >
      <ArrowLeft size={14} />
      返回单词本
    </button>
  </div>

  {#if errorMessage}
    <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
      {errorMessage}
    </div>
  {/if}

  {#if isLoading}
    <div class="text-sm text-base-content/60">加载中...</div>
  {:else if word}
    <div class="rounded-xl bg-base-100 border border-base-200 p-6">
      <div>
        <div class="text-2xl font-semibold flex items-center gap-3">
          <span>{word.term}</span>
          {#if word.phonetic}
            <span class="text-sm text-base-content/50">
              {word.phonetic}
            </span>
          {/if}
        </div>
        <div class="text-sm text-base-content/60 mt-1">
          {word.translation}
        </div>
        <div class="text-xs text-base-content/50 mt-1">
          {word.language}
        </div>
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div class="rounded-xl bg-base-100 border border-base-200 p-6">
        <h2 class="text-sm font-medium text-base-content/70 mb-3">简明解释</h2>
        <p class="text-sm text-base-content/70">
          {word.explanation || "暂无解释"}
        </p>
      </div>
      <div class="rounded-xl bg-base-100 border border-base-200 p-6">
        <h2 class="text-sm font-medium text-base-content/70 mb-3">备注</h2>
        <p class="text-sm text-base-content/70">
          {word.note || "暂无备注"}
        </p>
      </div>
    </div>
  {:else}
    <div class="text-sm text-base-content/60">未找到单词</div>
  {/if}
</div>
