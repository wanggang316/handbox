<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    createWord,
    listWords,
    deleteWord,
    getTranslationHistory,
  } from "$lib/api/word";
  import * as agentApi from "$lib/api/agent";
  import * as chatApi from "$lib/api/chat";
  import * as messageApi from "$lib/api/message";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { settingsState } from "$lib/states";
  import { providerActions, providerState } from "$lib/states/provider.svelte";
  import type { Word, Message } from "$lib/types";

  type TabId = "lookup" | "learn";

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
  ];

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
  let translationHistory = $state<Message[]>([]);

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
   * 创建或更新翻译 Session
   * 从 Agent 实例化 Session，拷贝所有配置（system_prompt 等）
   * 然后更新 model_id 和 provider_id
   */
  async function createOrUpdateTranslationSession(): Promise<string | null> {
    if (!agentId || !modelId || !providerId) {
      return null;
    }

    try {
      isUpdatingSession = true;
      const translation = settingsState.settings?.translation;
      const currentSessionId = translation?.sessionId;

      if (currentSessionId) {
        // Session 已存在，只需更新模型
        await chatApi.updateChatModel(currentSessionId, modelId, providerId);
        return currentSessionId;
      } else {
        // 从 Agent 创建 Session，自动拷贝 Agent 的所有配置
        const session = await chatApi.createSessionFromAgent(agentId);
        if (!session.id) {
          throw new Error("Failed to create session: no id returned");
        }
        // 更新用户选择的模型
        await chatApi.updateChatModel(session.id, modelId, providerId);

        // 保存 sessionId 到设置
        await settingsState.updateSettings({
          section: "translation",
          data: { sessionId: session.id },
        });

        return session.id;
      }
    } catch (error) {
      console.error("Failed to create/update translation session:", error);
      errorMessage = "创建翻译会话失败";
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
          errorMessage = "请先配置翻译 Agent 和模型";
          isLoading = false;
          return;
        }

        // 使用流式消息发送翻译请求
        let streamContent = "";
        await messageApi.sendUserMessageStream({
          chatId: sessionId,
          content: trimmed,
          tempUserMessageId: `trans-${Date.now()}`,
        });

        // 监听流式事件
        const unlisten = await messageApi.listenToStreamEvents({
          onChunk: (data) => {
            streamContent = data.content;
            lookupResult = {
              term: trimmed,
              translation: streamContent,
              sourceLanguage: "auto",
              targetLanguage: "unknown",
              phonetic: null,
              explanation: null,
              exists: false,
            };
          },
          onEnd: (data) => {
            // 解析翻译结果
            const result = parseTranslationResponse(data.finalContent, trimmed);
            lookupResult = {
              term: trimmed,
              translation: result.translation,
              sourceLanguage: "auto",
              targetLanguage: result.targetLanguage,
              phonetic: result.phonetic,
              explanation: result.explanation,
              exists: false,
            };
            isLoading = false;
          },
          onError: (error) => {
            console.error("Translation failed:", error);
            errorMessage = "翻译失败";
            isLoading = false;
          },
        });
      }
    } catch (error) {
      console.error("Failed to lookup word:", error);
      errorMessage = "查词失败";
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
      errorMessage = "添加单词失败";
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
      errorMessage = "删除单词失败";
    }
  }

  async function loadTranslationHistory() {
    try {
      const translation = settingsState.settings?.translation;
      const sessionId = translation?.sessionId;
      if (!sessionId) {
        translationHistory = [];
        return;
      }

      const messages = await getTranslationHistory(sessionId, 50, 0);
      // 只保留用户和助手的消息对
      translationHistory = messages.filter(
        (msg) => msg.role === "user" || msg.role === "assistant"
      );
    } catch (error) {
      console.error("Failed to load translation history:", error);
      translationHistory = [];
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
      errorMessage = "保存配置失败";
    }
  }

  async function handleAgentChange(value: string) {
    agentId = value;
    const agent = await agentApi.getAgent(value);
    selectedAgent = agent;

    // 如果 Agent 有配置的模型，使用 Agent 的模型
    if (agent.model) {
      for (const provider of providerState.providersWithModels) {
        const model = provider.models.find((m) => m.id === agent.model);
        if (model) {
          providerId = provider.id ?? "";
          modelId = agent.model;
          break;
        }
      }
    }
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

      // 如果已有 sessionId，从 session 加载配置
      // 但如果 modelId 和 providerId 已经存在，跳过 getChat 调用
      if (translation?.sessionId && (!modelId || !providerId)) {
        try {
          const t1 = performance.now();
          const session = await chatApi.getChat(translation.sessionId);
          console.log(`[Words] getChat: ${(performance.now() - t1).toFixed(2)}ms`);
          // 恢复 modelId 和 providerId
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

        <!-- 配置区域 -->
        <div class="flex flex-wrap items-center gap-3">
          <!-- Agent 选择 -->
          <div class="flex items-center gap-2">
            <span class="text-xs text-base-content/60">翻译 Agent</span>
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
            <ChatModelSelectButton
              {selectedModel}
              variant="gray"
              size="sm"
              onModelSelect={(model) => handleModelSelect(model)}
            />
          {/if}

          <button
            class="h-8 px-4 rounded-lg bg-primary text-base-100 text-sm"
            onclick={handleLookup}
            disabled={isLoading || !agentId || !modelId}
          >
            {isLoading ? "查询中..." : "查询"}
          </button>
        </div>

        <!-- 提示信息 -->
        {#if agentOptions.length === 0}
          <div class="text-xs text-base-content/60">
            暂无可用 Agent，请先在 Agent 管理页面创建翻译 Agent。
          </div>
        {:else if !agentId}
          <div class="text-xs text-base-content/60">
            请选择翻译 Agent
          </div>
        {:else if !modelId}
          <div class="text-xs text-base-content/60">
            请选择翻译模型
          </div>
        {/if}
      </div>
    </div>

    {#if translationHistory.length > 0}
      <div class="rounded-2xl bg-base-100 p-4 shadow-sm border border-base-200">
        <div class="text-xs text-base-content/60 mb-3">历史查询</div>
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
                  <div class="text-xs text-base-content/40">
                    {new Date(message.createdAt).toLocaleString()}
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
                <button
                  class="px-3 py-1 rounded-full text-xs bg-error/10 text-error"
                  onclick={(e) => {
                    e.stopPropagation();
                    handleDeleteWord(word.id);
                  }}
                >
                  删除
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
