<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { BookPlus, BookMinus } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import { createWord, listWords, deleteWord } from "$lib/api/word";
  import * as agentApi from "$lib/api/agent";
  import {
    createSessionFromDefinition,
    updateAgentSessionField,
    getAgentSession,
    getAgentSessionMessages,
    runAgentTextTurn,
    agentMessageText,
  } from "$lib/api/agentSession";
  import Select from "$lib/components/ui/Select.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import { settingsState } from "$lib/states";
  import { providerState } from "$lib/states/provider.svelte";
  import { normalizeError } from "$lib/utils/error";
  import type { Word } from "$lib/types";

  type TabId = "lookup" | "learn";

  /**
   * 翻译历史的归一化条目（取代旧的 chat `Message`）。源自统一 Agent 会话的
   * transcript（`getAgentSessionMessages` → `AgentSessionMessage.payload`），
   * 每条把助手/用户内容块拍平为可读字符串供历史列表与「加入单词本」消费。
   */
  type HistoryEntry = {
    id: string;
    role: "user" | "assistant";
    content: string;
    createdAt: number;
  };

  type LookupResult = {
    term: string;
    translation: string;
    sourceLanguage: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
    exists: boolean;
  };

  const tabs: Array<{ id: TabId; label: string }> = $derived([
    { id: "lookup", label: t("words.tab.lookup") },
    { id: "learn", label: t("words.tab.learn") },
  ]);

  let activeTab = $state<TabId>("lookup");
  let isLoading = $state(false);
  let isUpdatingSession = $state(false);
  let errorMessage = $state<string | null>(null);
  let words = $state<Word[]>([]);
  let listQuery = $state("");
  let lookupQuery = $state("");

  // 翻译配置
  let agentOptions = $state<{ value: string; label: string }[]>([]);
  let agentId = $state("");
  let selectedAgent = $state<any>(null);
  let providerId = $state("");
  let modelId = $state("");

  let lookupResult = $state<LookupResult | null>(null);
  let translationHistory = $state<HistoryEntry[]>([]);

  const selectedModel = $derived(
    (() => {
      if (!providerId || !modelId) return null;
      const provider = providerState.providersWithModels.find(
        (item) => item.id === providerId
      );
      const model = provider?.models.find((item) => item.id === modelId);
      if (!model || !provider || !provider.id) return null;
      return {
        ...model,
        providerName: provider.name,
        providerType: provider.provider_type,
        provider_id: provider.id,
      };
    })()
  );

  /**
   * 创建或更新翻译 Session（统一 Agent 引擎）。
   *
   * 已有会话：就地更新所选 model/provider（单字段更新，写回快照）。会话不存在
   * （settings 里残留的旧 chat session id —— 迁移前遗留）时退回到「从 definition 实例化
   * 一个新会话」并改写 settings.translation.sessionId，使旧 id 不会让翻译永久失效。
   * 无会话：直接从所选 AgentDefinition 实例化（覆盖 model/provider），其余配置由
   * definition 快照决定；translationAgent 的 workingDirMode 退化为纯对话。
   */
  async function createOrUpdateTranslationSession(): Promise<string | null> {
    if (!agentId || !modelId || !providerId) {
      return null;
    }

    try {
      isUpdatingSession = true;
      const currentSessionId = settingsState.settings?.translation?.sessionId;

      if (currentSessionId) {
        try {
          await updateAgentSessionField(currentSessionId, "modelId", modelId);
          await updateAgentSessionField(
            currentSessionId,
            "providerId",
            providerId
          );
          return currentSessionId;
        } catch (error) {
          // 旧 chat session id 残留（迁移前遗留）：落到下方实例化新会话。
          if (normalizeError(error).code !== "NOT_FOUND") {
            throw error;
          }
        }
      }

      // 从 AgentDefinition 实例化新会话，覆盖用户所选 model/provider。
      const session = await createSessionFromDefinition(agentId, {
        modelId,
        providerId,
      });
      await settingsState.updateSettings({
        section: "translation",
        data: { sessionId: session.id },
      });
      return session.id;
    } catch (error) {
      console.error("Failed to create/update translation session:", error);
      errorMessage = t("words.error.createSessionFailed");
      return null;
    } finally {
      isUpdatingSession = false;
    }
  }

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
      errorMessage = t("words.error.loadWordsFailed");
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
        isLoading = false;
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
          targetLanguage: exact.translation,
          phonetic: exact.phonetic,
          explanation: exact.explanation,
          exists: true,
        };
        isLoading = false;
      } else {
        const sessionId = await createOrUpdateTranslationSession();
        if (!sessionId) {
          errorMessage = t("words.error.configRequired");
          isLoading = false;
          return;
        }

        try {
          // 一问一答：发词条、聚合助手回复（增量实时回灌预览，结束再解析结构化结果）。
          const finalContent = await runAgentTextTurn(
            sessionId,
            trimmed,
            (partial) => {
              lookupResult = {
                term: trimmed,
                translation: partial,
                sourceLanguage: "auto",
                targetLanguage: "unknown",
                phonetic: null,
                explanation: null,
                exists: false,
              };
            }
          );
          const result = parseTranslationResponse(finalContent, trimmed);
          lookupResult = {
            term: trimmed,
            translation: result.translation,
            sourceLanguage: "auto",
            targetLanguage: result.targetLanguage,
            phonetic: result.phonetic,
            explanation: result.explanation,
            exists: false,
          };
          // run 把本回合 user/assistant 追加进 transcript，刷新历史列表。
          await loadTranslationHistory();
        } catch (error) {
          console.error("Translation failed:", error);
          errorMessage = t("words.error.translateFailed");
        } finally {
          isLoading = false;
        }
      }
    } catch (error) {
      console.error("Failed to lookup word:", error);
      errorMessage = t("words.error.lookupFailed");
      isLoading = false;
    }
  }

  /**
   * 解析翻译响应
   * 从 LLM 的 JSON 响应中提取翻译结果
   */
  function parseTranslationResponse(content: string, term: string): {
    term: string;
    translation: string;
    targetLanguage: string;
    phonetic: string | null;
    explanation: string | null;
  } {
    try {
      // 尝试解析 JSON 响应
      const jsonMatch = content.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        const parsed = JSON.parse(jsonMatch[0]);
        return {
          term,
          translation: parsed.translation || content,
          targetLanguage: parsed.targetLanguage || 'unknown',
          phonetic: parsed.phonetic || null,
          explanation: parsed.explanation || null,
        };
      }

      // 如果没有 JSON，直接返回内容作为翻译
      return {
        term,
        translation: content,
        targetLanguage: 'unknown',
        phonetic: null,
        explanation: null,
      };
    } catch (error) {
      console.error('Failed to parse translation response:', error);
      // 解析失败，返回原始内容
      return {
        term,
        translation: content,
        targetLanguage: 'unknown',
        phonetic: null,
        explanation: null,
      };
    }
  }

  async function handleAddLookup() {
    if (!lookupResult || lookupResult.exists || !lookupResult.translation) {
      return;
    }

    const currentLookup = lookupResult;
    try {
      isLoading = true;
      errorMessage = null;
      await createWord({
        term: currentLookup.term,
        translation: currentLookup.translation,
        language: currentLookup.sourceLanguage || "auto",
        phonetic: currentLookup.phonetic,
        explanation: currentLookup.explanation,
        source: "lookup",
      });
      lookupResult = { ...currentLookup, exists: true };
      await loadWords();
    } catch (error) {
      console.error("Failed to add lookup word:", error);
      errorMessage = t("words.error.addWordFailed");
    } finally {
      isLoading = false;
    }
  }

  async function handleDeleteWord(wordId: string) {
    try {
      await deleteWord(wordId);
      await loadWords();
    } catch (error) {
      console.error("Failed to delete word:", error);
      errorMessage = t("words.error.deleteWordFailed");
    }
  }

  async function loadTranslationHistory() {
    try {
      const sessionId = settingsState.settings?.translation?.sessionId;
      if (!sessionId) {
        translationHistory = [];
        return;
      }

      // 统一 Agent 会话的 transcript（seq ASC，即时间正序）。拍平内容块取纯文本，
      // 按 (user→assistant) 成对，再整体倒序使最新一对在前——模板按 index/index+1
      // 配对渲染，故对内仍须 user 在前、assistant 紧随。
      const messages = await getAgentSessionMessages(sessionId);
      const chrono: HistoryEntry[] = messages
        .filter(
          (msg) =>
            msg.payload.role === "user" || msg.payload.role === "assistant"
        )
        .map((msg) => ({
          id: msg.id,
          role: msg.payload.role as "user" | "assistant",
          content: agentMessageText(msg.payload),
          createdAt: msg.createdAt,
        }));

      const pairs: HistoryEntry[][] = [];
      for (let i = 0; i < chrono.length; i++) {
        if (chrono[i].role === "user" && chrono[i + 1]?.role === "assistant") {
          pairs.push([chrono[i], chrono[i + 1]]);
          i++;
        }
      }
      translationHistory = pairs.reverse().flat();
    } catch (error) {
      console.error("Failed to load translation history:", error);
      translationHistory = [];
    }
  }

  async function handleAddFromHistory(
    userEntry: HistoryEntry,
    assistantEntry: HistoryEntry
  ) {
    try {
      isLoading = true;
      errorMessage = null;

      // 尝试解析助手消息的 JSON 响应
      const parsed = parseTranslationResponse(
        assistantEntry.content,
        userEntry.content
      );

      await createWord({
        term: userEntry.content,
        translation: parsed.translation,
        language: parsed.targetLanguage || "auto",
        phonetic: parsed.phonetic,
        explanation: parsed.explanation,
        source: "history",
      });

      await loadWords();
    } catch (error) {
      console.error("Failed to add word from history:", error);
      errorMessage = t("words.error.addWordFailed");
    } finally {
      isLoading = false;
    }
  }

  /**
   * 检查某个词是否已在单词本中
   */
  function isWordInWordbook(term: string): boolean {
    return words.some((word) => word.term.toLowerCase() === term.toLowerCase());
  }

  /**
   * 从单词本中移除某个词
   */
  async function handleRemoveFromHistory(term: string) {
    try {
      const word = words.find((w) => w.term.toLowerCase() === term.toLowerCase());
      if (word) {
        await deleteWord(word.id);
        await loadWords();
      }
    } catch (error) {
      console.error("Failed to remove word from wordbook:", error);
      errorMessage = t("words.error.removeWordFailed");
    }
  }

  async function loadAgents() {
    try {
      const agents = await agentApi.getAgents(100, 0);
      agentOptions = agents
        .filter((agent) => agent.id)
        .map((agent) => ({
          value: agent.id!,
          label: agent.name,
        }));
    } catch (error) {
      console.error("Failed to load agents:", error);
    }
  }

  async function loadProviders() {
    // providers 已在根布局预加载，直接使用 providerState.providersWithModels
    // 无需再次调用 API
    console.log('[Words] loadProviders: using cached providers');
  }

  async function saveConfig() {
    try {
      await createOrUpdateTranslationSession();
      // sessionId 在 createOrUpdateTranslationSession 中已保存
      errorMessage = null;
    } catch (error) {
      console.error("Failed to save config:", error);
      errorMessage = t("words.error.saveConfigFailed");
    }
  }

  async function handleAgentChange(value: string) {
    agentId = value;
    const agent = await agentApi.getAgent(value);
    selectedAgent = agent;
    // The Agent no longer carries a model — the model is chosen separately on
    // this page (modelId / providerId), so selecting an Agent doesn't touch it.
    await saveConfig();
  }

  async function handleModelSelect(model: any) {
    providerId = model.provider_id;
    modelId = model.id;
    await saveConfig();
  }

  async function loadSessionFromSettings() {
    const t = performance.now();
    try {
      const translation = settingsState.settings?.translation;
      console.log(`[Words] translation:`, translation);

      // 设置默认 agent
      if (agentOptions.length > 0 && !agentId) {
        agentId = agentOptions[0].value;
      }

      // 如果已有 sessionId，从 Agent 会话快照恢复 model/provider；
      // model/provider 已就位则跳过。会话不存在（旧 chat id 残留）只记录并跳过，
      // 配置回退到 agent 默认（handleAgentChange）。
      if (translation?.sessionId && (!modelId || !providerId)) {
        try {
          const t1 = performance.now();
          const session = await getAgentSession(translation.sessionId);
          console.log(
            `[Words] getAgentSession: ${(performance.now() - t1).toFixed(2)}ms`
          );
          if (session.modelId) {
            modelId = session.modelId;
          }
          if (session.providerId) {
            providerId = session.providerId;
          }
        } catch (error) {
          console.error("Failed to load session:", error);
        }
      }
    } catch (error) {
      console.error("Failed to load session from settings:", error);
    }
    console.log(`[Words] loadSessionFromSettings: ${(performance.now() - t).toFixed(2)}ms`);
  }

  onMount(async () => {
    const t0 = performance.now();
    console.log('[Words] onMount started');

    // 并行加载独立的数据，提高加载速度
    await Promise.all([
      (async () => {
        const t = performance.now();
        await loadAgents();
        console.log(`[Words] loadAgents: ${(performance.now() - t).toFixed(2)}ms`);
      })(),
      (async () => {
        const t = performance.now();
        await loadProviders();
        console.log(`[Words] loadProviders: ${(performance.now() - t).toFixed(2)}ms`);
      })(),
      (async () => {
        const t = performance.now();
        await loadWords();
        console.log(`[Words] loadWords: ${(performance.now() - t).toFixed(2)}ms`);
      })(),
      (async () => {
        const t = performance.now();
        await settingsState.loadSettings(); // 现在有缓存，不会重复请求
        console.log(`[Words] loadSettings: ${(performance.now() - t).toFixed(2)}ms`);
      })(),
      (async () => {
        const t = performance.now();
        await loadTranslationHistory();
        console.log(`[Words] loadTranslationHistory: ${(performance.now() - t).toFixed(2)}ms`);
      })(),
    ]);

    console.log(`[Words] Promise.all done: ${(performance.now() - t0).toFixed(2)}ms`);

    // 加载完基础设置后，再从 session 恢复配置（如果需要）
    // 这个 getChat 调用可能还是有点慢，但它是必须的
    await loadSessionFromSettings();
    console.log(`[Words] onMount total: ${(performance.now() - t0).toFixed(2)}ms`);
  });
</script>

<div class="h-full flex flex-col gap-4 p-6">
  <div class="flex items-center justify-between">
    <div>
      <h1 class="text-xl font-medium text-base-content">{t("words.title")}</h1>
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
    <div class="rounded-lg bg-base-300 p-4 border border-[var(--hairline)]">
      <div class="flex flex-col md:flex-row gap-3">
        <input
          class="flex-1 h-10 rounded-lg bg-base-200 border border-[var(--hairline)] px-3 text-sm outline-none"
          placeholder={t("words.listSearchPlaceholder")}
          bind:value={listQuery}
          onkeydown={(event) => event.key === "Enter" && loadWords()}
        />
        <button
          class="h-10 px-4 rounded-lg bg-base-300 text-sm"
          onclick={loadWords}
          disabled={isLoading}
        >
          {t("common.search")}
        </button>
      </div>
    </div>
  {/if}

  {#if activeTab === "lookup"}
    <div class="rounded-lg bg-base-300 p-4 border border-[var(--hairline)]">
      <div class="flex flex-col gap-3">
        <textarea
          class="w-full h-20 rounded-lg bg-base-200 border border-[var(--hairline)] px-3 py-2 text-sm outline-none resize-none"
          rows={2}
          placeholder={t("words.lookupPlaceholder")}
          bind:value={lookupQuery}
          onkeydown={(event) =>
            event.key === "Enter" && !event.shiftKey && handleLookup()}
        ></textarea>

        <!-- 配置区域 -->
        <div class="flex flex-wrap items-center gap-3">
          <!-- Agent 选择 -->
          <div class="flex items-center gap-2">
            <span class="text-xs text-base-content/60">{t("words.translationAgent")}</span>
            <Select
              options={agentOptions}
              bind:selectedValue={agentId}
              onChange={(value) => handleAgentChange(value)}
              size="sm"
              disabled={isUpdatingSession}
            />
          </div>

          <!-- 模型选择（Agent 选择后显示） -->
          {#if agentId}
            <div class="text-xs text-base-content/50">
              {selectedModel?.name ?? "No model"}
            </div>
          {/if}

          <button
            class="h-8 px-4 rounded-lg bg-primary text-base-100 text-sm"
            onclick={handleLookup}
            disabled={isLoading || !agentId || !modelId}
          >
            {isLoading ? t("words.querying") : t("words.query")}
          </button>
        </div>

        <!-- 提示信息 -->
        {#if agentOptions.length === 0}
          <div class="text-xs text-base-content/60">
            {t("words.noAgentHint")}
          </div>
        {:else if !agentId}
          <div class="text-xs text-base-content/60">
            {t("words.selectAgentHint")}
          </div>
        {:else if !modelId}
          <div class="text-xs text-base-content/60">
            {t("words.selectModelHint")}
          </div>
        {/if}
      </div>
    </div>

    {#if translationHistory.length > 0}
      <div class="rounded-lg bg-base-300 p-4 border border-[var(--hairline)]">
        <div class="text-xs text-base-content/60 mb-3">{t("words.history")}</div>
        <div class="divide-y divide-base-200 max-h-96 overflow-y-auto">
          {#each translationHistory as message, index}
            {#if message.role === "user" && translationHistory[index + 1]?.role === "assistant"}
              <div class="py-3">
                <div class="flex flex-col gap-2">
                  <div class="text-sm font-medium text-base-content">
                    {message.content}
                  </div>
                  <div class="text-sm text-base-content/70">
                    {translationHistory[index + 1].content}
                  </div>
                  <div class="flex items-center justify-between gap-2">
                    <div class="text-xs text-base-content/40">
                      {new Date(message.createdAt).toLocaleString()}
                    </div>
                    <div class="flex items-center gap-2">
                      {#if isWordInWordbook(message.content)}
                        <IconButton
                          icon={BookMinus}
                          iconSize={16}
                          title={t("words.removeFromWordbook")}
                          disabled={isLoading}
                          onclick={() => handleRemoveFromHistory(message.content)}
                        />
                      {:else}
                        <IconButton
                          icon={BookPlus}
                          iconSize={16}
                          title={t("words.addToWordbook")}
                          disabled={isLoading}
                          onclick={() => handleAddFromHistory(message, translationHistory[index + 1])}
                        />
                      {/if}
                    </div>
                  </div>
                </div>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  {/if}

  {#if activeTab !== "lookup"}
    <div
      class="flex-1 overflow-auto rounded-lg bg-base-300 border border-[var(--hairline)]"
    >
      {#if isLoading}
        <div class="p-6 text-sm text-base-content/60">{t("common.loading")}</div>
      {:else if words.length === 0}
        <div class="p-6 text-sm text-base-content/60">{t("words.emptyList")}</div>
      {:else}
        <div class="divide-y divide-base-200">
          {#each words as word}
            <div
              class="p-4 flex flex-col gap-3 hover:bg-base-300/60 cursor-pointer"
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
                <button
                  class="px-3 py-1 rounded-full text-xs bg-error/10 text-error"
                  onclick={(e) => {
                    e.stopPropagation();
                    handleDeleteWord(word.id);
                  }}
                >
                  {t("common.delete")}
                </button>
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
