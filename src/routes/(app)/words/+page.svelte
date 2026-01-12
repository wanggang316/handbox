<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    createWord,
    deleteLookupHistory,
    listLookupHistory,
    listWords,
    recordLookup,
    reviewWord,
    deleteWord,
    translateWord,
  } from "$lib/api/word";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import LookupResultRow from "$lib/components/words/LookupResultRow.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { settingsState } from "$lib/states";
  import { providerActions, providerState } from "$lib/states/provider.svelte";
  import type { Word } from "$lib/types";
  import { Trash2 } from "@lucide/svelte";

  type TabId = "lookup" | "learn" | "review";

  type LookupResult = {
    term: string;
    translation: string;
    sourceLanguage: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
    exists: boolean;
  };

  const tabs: Array<{ id: TabId; label: string }> = [
    { id: "lookup", label: "查词" },
    { id: "learn", label: "学习" },
    { id: "review", label: "复习" },
  ];

  let activeTab = $state<TabId>("lookup");
  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let words = $state<Word[]>([]);
  let listQuery = $state("");
  let lookupQuery = $state("");
  let translationProviderId = $state("");
  let translationModelId = $state("");
  let targetLanguage = $state("system");
  let customTargetLanguage = $state("");

  let lookupResult = $state<LookupResult | null>(null);
  let lookupHistory = $state<
    Array<{
      id: string;
      term: string;
      translation?: string | null;
      phonetic?: string | null;
      explanation?: string | null;
      sourceLanguage?: string | null;
      targetLanguage?: string | null;
      exists?: boolean;
      createdAt: number;
    }>
  >([]);

  const targetLanguageOptions = [
    { value: "system", label: "跟随系统" },
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
    { value: "ja-JP", label: "日本語" },
    { value: "ko-KR", label: "한국어" },
    { value: "fr-FR", label: "Français" },
    { value: "de-DE", label: "Deutsch" },
    { value: "es-ES", label: "Español" },
    { value: "it-IT", label: "Italiano" },
    { value: "ru-RU", label: "Русский" },
    { value: "pt-BR", label: "Português (BR)" },
    { value: "ar-SA", label: "العربية" },
    { value: "custom", label: "自定义" },
  ];

  const selectedModel = $derived(
    (() => {
      if (!translationProviderId || !translationModelId) return null;
      const provider = providerState.providersWithModels.find(
        (item) => item.id === translationProviderId
      );
      const model = provider?.models.find(
        (item) => item.id === translationModelId
      );
      if (!model || !provider) return null;
      return {
        ...model,
        providerName: provider.name,
        providerType: provider.provider_type,
      };
    })()
  );

  async function loadWords() {
    try {
      isLoading = true;
      errorMessage = null;
      words = await listWords({
        query: listQuery.trim() || undefined,
        limit: 100,
        offset: 0,
      });
      syncLookupHistoryWithWords();
    } catch (error) {
      console.error("Failed to load words:", error);
      errorMessage = "加载单词失败";
    } finally {
      isLoading = false;
    }
  }

  function syncLookupHistoryWithWords() {
    if (!lookupHistory.length) return;
    const existing = new Set(
      words.map((word) => word.term.trim().toLowerCase())
    );
    lookupHistory = lookupHistory.map((item) => ({
      ...item,
      exists: existing.has(item.term.trim().toLowerCase()),
    }));
  }

  async function handleLookup() {
    try {
      isLoading = true;
      errorMessage = null;
      const trimmed = lookupQuery.trim();
      if (!trimmed) {
        lookupResult = null;
        return;
      }

      const results = await listWords({
        query: trimmed,
        limit: 20,
        offset: 0,
      });
      const exact = results.find(
        (word) => word.term.toLowerCase() === trimmed.toLowerCase()
      );

      if (exact) {
        lookupResult = {
          term: exact.term,
          translation: exact.translation,
          sourceLanguage: exact.language,
          targetLanguage: targetLanguage,
          phonetic: exact.phonetic,
          explanation: exact.explanation,
          exists: true,
        };
      } else {
        if (!translationProviderId || !translationModelId) {
          errorMessage = "请先选择翻译模型";
          return;
        }
        const translation = await translateWord({ term: trimmed });
        lookupResult = {
          term: trimmed,
          translation: translation.translation,
          sourceLanguage: "auto",
          targetLanguage: translation.targetLanguage,
          phonetic: translation.phonetic,
          explanation: translation.explanation,
          exists: false,
        };
      }

      const recorded = await recordLookup({
        term: lookupResult.term,
        translation: lookupResult.translation,
        phonetic: lookupResult.phonetic,
        explanation: lookupResult.explanation,
        sourceLanguage: lookupResult.sourceLanguage,
        targetLanguage: lookupResult.targetLanguage,
      });
      lookupHistory = [
        {
          ...recorded,
          exists: words.some(
            (word) =>
              word.term.trim().toLowerCase() ===
              recorded.term.trim().toLowerCase()
          ),
        },
        ...lookupHistory,
      ];
    } catch (error) {
      console.error("Failed to lookup word:", error);
      errorMessage = "查词失败";
    } finally {
      isLoading = false;
    }
  }

  async function handleAddLookup() {
    if (!lookupResult || lookupResult.exists || !lookupResult.translation) {
      return;
    }

    try {
      isLoading = true;
      errorMessage = null;
      await createWord({
        term: lookupResult.term,
        translation: lookupResult.translation,
        language: lookupResult.sourceLanguage || "auto",
        phonetic: lookupResult.phonetic,
        explanation: lookupResult.explanation,
        source: "lookup",
      });
      lookupResult = { ...lookupResult, exists: true };
      await loadWords();
      lookupHistory = lookupHistory.map((item) =>
        item.term.trim().toLowerCase() ===
        lookupResult.term.trim().toLowerCase()
          ? { ...item, exists: true }
          : item
      );
    } catch (error) {
      console.error("Failed to add lookup word:", error);
      errorMessage = "添加单词失败";
    } finally {
      isLoading = false;
    }
  }

  async function handleReview(wordId: string, remembered: boolean) {
    try {
      await reviewWord({ wordId, remembered });
      await loadWords();
    } catch (error) {
      console.error("Failed to review word:", error);
      errorMessage = "更新复习失败";
    }
  }

  async function handleDeleteWord(wordId: string) {
    try {
      await deleteWord(wordId);
      await loadWords();
    } catch (error) {
      console.error("Failed to delete word:", error);
      errorMessage = "删除单词失败";
    }
  }

  async function handleDeleteLookup(historyId: string) {
    try {
      await deleteLookupHistory(historyId);
      lookupHistory = lookupHistory.filter((item) => item.id !== historyId);
    } catch (error) {
      console.error("Failed to delete lookup history:", error);
      errorMessage = "删除查询记录失败";
    }
  }

  async function handleAddHistory(item: {
    term: string;
    translation?: string | null;
    phonetic?: string | null;
    explanation?: string | null;
    sourceLanguage?: string | null;
    exists?: boolean;
  }) {
    if (!item.translation || item.exists) {
      errorMessage = "查询结果不完整，无法添加";
      return;
    }
    try {
      await createWord({
        term: item.term,
        translation: item.translation,
        language: item.sourceLanguage || "auto",
        phonetic: item.phonetic,
        explanation: item.explanation,
        source: "lookup",
      });
      await loadWords();
      lookupHistory = lookupHistory.map((history) =>
        history.id === item.id ? { ...history, exists: true } : history
      );
    } catch (error) {
      console.error("Failed to add history word:", error);
      errorMessage = "添加单词失败";
    }
  }

  function handleActionClick(event: MouseEvent) {
    event.stopPropagation();
  }

  async function loadTranslationSettings() {
    try {
      await settingsState.loadSettings();
      await providerActions.loadProvidersWithModels(false);
      const translation = settingsState.settings?.translation;
      translationProviderId = translation?.providerId || "";
      translationModelId = translation?.modelId || "";
      const savedTarget = translation?.targetLanguage || "system";
      const hasPreset = targetLanguageOptions.some(
        (option) => option.value === savedTarget
      );
      if (hasPreset) {
        targetLanguage = savedTarget;
      } else {
        targetLanguage = "custom";
        customTargetLanguage = savedTarget;
      }
    } catch (error) {
      console.error("Failed to load translation settings:", error);
    }
  }

  async function updateTranslationSettings(data: Record<string, unknown>) {
    try {
      await settingsState.updateSettings({
        section: "translation",
        data,
      });
    } catch (error) {
      console.error("Failed to update translation settings:", error);
      errorMessage = "保存设置失败";
    }
  }

  async function handleTargetLanguageChange(value: string) {
    targetLanguage = value;
    if (value !== "custom") {
      await updateTranslationSettings({ targetLanguage: value });
    }
  }

  async function handleCustomTargetChange(value: string) {
    customTargetLanguage = value;
    if (targetLanguage === "custom") {
      await updateTranslationSettings({
        targetLanguage: customTargetLanguage || "system",
      });
    }
  }

  async function handleModelSelect(model: any) {
    translationProviderId = model.provider_id;
    translationModelId = model.id;
    await updateTranslationSettings({
      providerId: translationProviderId,
      modelId: translationModelId,
    });
  }

  onMount(async () => {
    await loadWords();
    await loadTranslationSettings();
    try {
      lookupHistory = await listLookupHistory({ limit: 20, offset: 0 });
      syncLookupHistoryWithWords();
    } catch (error) {
      console.error("Failed to load lookup history:", error);
    }
  });
</script>

<div class="h-full flex flex-col gap-4 p-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-base-content">单词本</h1>
    </div>
  </div>

  <div class="flex items-center gap-2">
    {#each tabs as tab}
      <button
        class="px-3 py-1.5 rounded-full text-sm border"
        class:bg-primary={activeTab === tab.id}
        class:text-base-100={activeTab === tab.id}
        class:border-primary={activeTab === tab.id}
        class:border-base-300={activeTab !== tab.id}
        class:text-base-content={activeTab !== tab.id}
        onclick={() => (activeTab = tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  {#if errorMessage}
    <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
      {errorMessage}
    </div>
  {/if}

  {#if activeTab !== "lookup"}
    <div class="rounded-2xl bg-base-100 p-4 shadow-sm border border-base-200">
      <div class="flex flex-col md:flex-row gap-3">
        <input
          class="flex-1 h-10 rounded-lg bg-base-200 px-3 text-sm outline-none"
          placeholder="搜索单词或释义"
          bind:value={listQuery}
          onkeydown={(event) => event.key === "Enter" && loadWords()}
        />
        <button
          class="h-10 px-4 rounded-lg bg-base-300 text-sm"
          onclick={loadWords}
          disabled={isLoading}
        >
          搜索
        </button>
      </div>
    </div>
  {/if}

  {#if activeTab === "lookup"}
    <div class="rounded-2xl bg-base-100 p-4 shadow-sm border border-base-200">
      <div class="flex flex-col gap-3">
        <textarea
          class="w-full h-20 rounded-lg bg-base-200 px-3 py-2 text-sm outline-none resize-none"
          rows={2}
          placeholder="输入单词、短语或句子"
          bind:value={lookupQuery}
          onkeydown={(event) =>
            event.key === "Enter" && !event.shiftKey && handleLookup()}
        ></textarea>
        <div class="flex flex-wrap items-center gap-3">
          <div class="flex items-center gap-2">
            <span class="text-xs text-base-content/60">目标语言</span>
            <Select
              options={targetLanguageOptions}
              bind:selectedValue={targetLanguage}
              onChange={(value) => handleTargetLanguageChange(value)}
              size="sm"
            />
          </div>
          {#if targetLanguage === "custom"}
            <input
              class="h-8 rounded-lg bg-base-200 px-2 text-xs outline-none"
              placeholder="语言标签，如 en-US"
              bind:value={customTargetLanguage}
              oninput={(event) =>
                handleCustomTargetChange(
                  (event.target as HTMLInputElement).value
                )}
            />
          {/if}
          <ChatModelSelectButton
            {selectedModel}
            variant="gray"
            size="sm"
            onModelSelect={(model) => handleModelSelect(model)}
          />
          <button
            class="h-8 px-4 rounded-lg bg-primary text-base-100 text-sm"
            onclick={handleLookup}
            disabled={isLoading ||
              !translationProviderId ||
              !translationModelId}
          >
            {isLoading ? "查询中..." : "查询"}
          </button>
        </div>
      </div>
    </div>

    {#if lookupHistory.length > 0}
      <div class="rounded-2xl bg-base-100 p-4 shadow-sm border border-base-200">
        <div class="text-xs text-base-content/60 mb-3">历史查询</div>
        <div class="divide-y divide-base-200">
          {#each lookupHistory as item}
            <div class="py-3">
              <LookupResultRow
                {item}
                busy={isLoading}
                showDelete={true}
                onAdd={() => handleAddHistory(item)}
                onDelete={() => handleDeleteLookup(item.id)}
              />
            </div>
          {/each}
        </div>
      </div>
    {/if}
  {/if}

  {#if activeTab !== "lookup"}
    <div
      class="flex-1 overflow-auto rounded-2xl bg-base-100 border border-base-200"
    >
      {#if isLoading}
        <div class="p-6 text-sm text-base-content/60">加载中...</div>
      {:else if words.length === 0}
        <div class="p-6 text-sm text-base-content/60">暂无单词</div>
      {:else}
        <div class="divide-y divide-base-200">
          {#each words as word}
            <div
              class="p-4 flex flex-col gap-3 hover:bg-base-200/40 cursor-pointer"
              onclick={() => goto(`/words/${word.id}`)}
            >
              <div
                class="flex flex-col md:flex-row md:items-center md:justify-between gap-3"
              >
                <div class="flex-1">
                  <div class="text-base font-medium flex items-center gap-2">
                    <span>{word.term}</span>
                    {#if word.phonetic}
                      <span class="text-xs text-base-content/50">
                        {word.phonetic}
                      </span>
                    {/if}
                  </div>
                <div class="text-sm text-base-content/60">
                  {word.translation}
                </div>
                {#if word.explanation}
                  <div class="text-xs text-base-content/50 mt-1">
                    {word.explanation}
                  </div>
                {/if}
              </div>
                <div
                  class="flex items-center gap-2"
                  onclick={handleActionClick}
                >
                  <button
                    class="px-3 py-1 rounded-full text-xs bg-success/10 text-success"
                    onclick={() => handleReview(word.id, true)}
                  >
                    记住
                  </button>
                  <button
                    class="px-3 py-1 rounded-full text-xs bg-warning/10 text-warning"
                    onclick={() => handleReview(word.id, false)}
                  >
                    忘记
                  </button>
                  <button
                    class="px-3 py-1 rounded-full text-xs bg-error/10 text-error"
                    onclick={() => handleDeleteWord(word.id)}
                  >
                    删除
                  </button>
                </div>
              </div>
              <div class="flex items-center gap-2 text-xs text-base-content/50">
                <span>{word.language}</span>
                <span>·</span>
                <span>{new Date(word.updatedAt).toLocaleDateString()}</span>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
