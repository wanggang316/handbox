<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeft } from "@lucide/svelte";
  import { getWord, reviewWord } from "$lib/api/word";
  import type { WordDetail } from "$lib/types";

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let detail = $state<WordDetail | null>(null);

  const wordId = $derived($page.params.id);

  async function loadDetail() {
    if (!wordId) {
      errorMessage = "无效的单词 ID";
      return;
    }
    try {
      isLoading = true;
      errorMessage = null;
      detail = await getWord(wordId);
    } catch (error) {
      console.error("Failed to load word detail:", error);
      errorMessage = "加载详情失败";
    } finally {
      isLoading = false;
    }
  }

  async function handleReview(remembered: boolean) {
    if (!detail) return;
    try {
      await reviewWord({ wordId: detail.word.id, remembered });
      await loadDetail();
    } catch (error) {
      console.error("Failed to review word:", error);
      errorMessage = "更新复习失败";
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
  {:else if detail}
    <div class="rounded-2xl bg-base-100 border border-base-200 p-6">
      <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
        <div>
          <div class="text-2xl font-semibold flex items-center gap-3">
            <span>{detail.word.term}</span>
            {#if detail.word.phonetic}
              <span class="text-sm text-base-content/50">
                {detail.word.phonetic}
              </span>
            {/if}
          </div>
          <div class="text-sm text-base-content/60 mt-1">
            {detail.word.translation}
          </div>
          <div class="text-xs text-base-content/50 mt-1">
            {detail.word.language}
          </div>
        </div>
        <div class="flex gap-2">
          <button
            class="px-3 py-1 rounded-full text-xs bg-success/10 text-success"
            onclick={() => handleReview(true)}
          >
            记住
          </button>
          <button
            class="px-3 py-1 rounded-full text-xs bg-warning/10 text-warning"
            onclick={() => handleReview(false)}
          >
            忘记
          </button>
        </div>
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
      <div class="rounded-2xl bg-base-100 border border-base-200 p-6">
        <h2 class="text-sm font-medium text-base-content/70 mb-3">简明解释</h2>
        <p class="text-sm text-base-content/70">
          {detail.word.explanation || "暂无解释"}
        </p>
      </div>
      <div class="rounded-2xl bg-base-100 border border-base-200 p-6">
        <h2 class="text-sm font-medium text-base-content/70 mb-3">备注</h2>
        <p class="text-sm text-base-content/70">
          {detail.word.note || "暂无备注"}
        </p>
      </div>

      <div class="rounded-2xl bg-base-100 border border-base-200 p-6">
        <h2 class="text-sm font-medium text-base-content/70 mb-3">复习记录</h2>
        {#if detail.review}
          <div class="text-sm text-base-content/70 space-y-1">
            <div>复习次数：{detail.review.reviewCount}</div>
            <div>间隔天数：{detail.review.intervalDays}</div>
            <div>下次复习：{new Date(detail.review.nextReviewAt).toLocaleString()}</div>
          </div>
        {:else}
          <div class="text-sm text-base-content/60">暂无复习记录</div>
        {/if}
      </div>
    </div>

    <div class="rounded-2xl bg-base-100 border border-base-200 p-6">
      <h2 class="text-sm font-medium text-base-content/70 mb-3">上下文</h2>
      {#if detail.contexts.length === 0}
        <div class="text-sm text-base-content/60">暂无上下文</div>
      {:else}
        <div class="space-y-3">
          {#each detail.contexts as context}
            <div class="p-3 rounded-lg bg-base-200/50 text-sm text-base-content/70">
              {context.contextText}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <div class="text-sm text-base-content/60">未找到单词</div>
  {/if}
</div>
