<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { createWord, listWords, reviewWord, translateWord } from "$lib/api/word";
  import ChatModelSelectModal from "$lib/components/chat/ChatModelSelectModal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { settingsState } from "$lib/states";
  import { providerActions, providerState } from "$lib/states/provider.svelte";
  import type { Word } from "$lib/types";
  import { ChevronsUpDown } from "@lucide/svelte";

  type TabId = "lookup" | "learn" | "review";

  type LookupResult = {
    term: string;
    translation: string;
    sourceLanguage: string;
    targetLanguage: string;
    phonetic?: string | null;
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
  let showModelModal = $state(false);
  let translationProviderId = $state("");
  let translationModelId = $state("");
  let targetLanguage = $state("system");
  let customTargetLanguage = $state("");

  let lookupResult = $state<LookupResult | null>(null);

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

  const selectedModel = $derived(() => {
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
  });

  async function loadWords() {
    try {
      isLoading = true;
      errorMessage = null;
      words = await listWords({
        query: listQuery.trim() || undefined,
        limit: 100,
        offset: 0,
      });
    } catch (error) {
      console.error("Failed to load words:", error);
      errorMessage = "加载单词失败";
    } finally {
      isLoading = false;
    }
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
          targetLanguage: "",
          phonetic: exact.phonetic,
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
          phonetic: null,
          exists: false,
        };
      }
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
        source: "lookup",
      });
      lookupResult = { ...lookupResult, exists: true };
      await loadWords();
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
  });
</script>

<div class="h-full flex flex-col gap-4 p-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-xl font-semibold text-base-content">单词本</h1>
      <p class="text-sm text-base-content/60">管理、学习与复习你的单词</p>
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
          onkeydown={(event) => event.key === "Enter" && !event.shiftKey && handleLookup()}
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
                handleCustomTargetChange((event.target as HTMLInputElement).value)
              }
            />
          {/if}
          <Button variant="clear" size="sm" onclick={() => (showModelModal = true)}>
            {selectedModel ? selectedModel.name : "选择模型"}
            <ChevronsUpDown size={14} />
          </Button>
          <button
            class="h-8 px-4 rounded-lg bg-primary text-base-100 text-sm"
            onclick={handleLookup}
            disabled={isLoading || !translationProviderId || !translationModelId}
          >
            查询
          </button>
        </div>
      </div>

      {#if lookupResult}
        <div class="mt-4 rounded-xl border border-base-200 bg-base-200/30 p-4">
          <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-3">
            <div>
              <div class="text-base font-medium flex items-center gap-2">
                <span>{lookupResult.term}</span>
                {#if lookupResult.phonetic}
                  <span class="text-xs text-base-content/50">
                    {lookupResult.phonetic}
                  </span>
                {/if}
              </div>
              <div class="text-sm text-base-content/60">
                {lookupResult.translation || "等待翻译结果"}
              </div>
              <div class="text-xs text-base-content/50 mt-1">
                {lookupResult.targetLanguage || lookupResult.sourceLanguage}
              </div>
            </div>
            <div>
              {#if lookupResult.exists}
                <span class="text-xs text-base-content/60">已在单词本</span>
              {:else}
                <button
                  class="px-3 py-1 rounded-full text-xs bg-primary text-base-100"
                  onclick={handleAddLookup}
                  disabled={!lookupResult.translation || isLoading}
                >
                  添加到单词本
                </button>
              {/if}
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {#if activeTab !== "lookup"}
    <div class="flex-1 overflow-auto rounded-2xl bg-base-100 border border-base-200">
      {#if isLoading}
        <div class="p-6 text-sm text-base-content/60">加载中...</div>
      {:else if words.length === 0}
        <div class="p-6 text-sm text-base-content/60">暂无单词</div>
      {:else}
        <div class="divide-y divide-base-200">
          {#each words as word}
            <div
              class="p-4 flex flex-col md:flex-row md:items-center md:justify-between gap-3 hover:bg-base-200/40 cursor-pointer"
              onclick={() => goto(`/words/${word.id}`)}
            >
              <div>
                <div class="text-base font-medium">{word.term}</div>
                <div class="text-sm text-base-content/60">
                  {word.translation}
                </div>
              </div>
              <div class="flex items-center gap-2">
                <span class="text-xs text-base-content/50">{word.language}</span>
                <div class="flex items-center gap-2" onclick={handleActionClick}>
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
                </div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={selectedModel}
  onModelSelect={(model) => handleModelSelect(model)}
/>
