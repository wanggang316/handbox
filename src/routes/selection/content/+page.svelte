<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import {
    Eye,
    Languages,
    Sparkles,
    X,
    Pin,
    PinOff,
    Copy,
    RotateCcw,
    MessageCirclePlus,
    ChevronDown,
  } from "@lucide/svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { hideContentPanel, setContentPanelPinned } from "$lib/api/selection";
  import { settingsState } from "$lib/states/settings.svelte";
  import {
    providerActions,
    getAllModels,
  } from "$lib/states/provider.svelte";
  import { resolveQuickActionModel } from "$lib/quickaction/resolveModel";
  import { t } from "$lib/i18n";
  import {
    runAgentTextTurn,
    createSessionFromDefinition,
    updateAgentSessionField,
  } from "$lib/api/agentSession";
  import {
    resolveSpec,
    looksLikeStreamingSpec,
  } from "$lib/components/genui/jsonui/resolveSpec";
  import { uiRegistry } from "$lib/components/genui/jsonui/registry";
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import type { Spec } from "@json-render/core";

  const appWindow = getCurrentWindow();

  type TranslationResult = {
    term: string;
    translation: string;
    targetLanguage: string;
    phonetic: string | null;
    explanation: string | null;
  };

  // 内容状态
  let content = $state({
    mode: "" as "show" | "translate" | "ai" | "",
    text: "",
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  // 翻译状态。spec 与 result 互斥：最终回复是合法 JSON-Render spec 时走 GenUI
  // 卡片（spec），否则回落结构化/纯文本解析（result）。
  let translation = $state({
    isLoading: false,
    result: null as TranslationResult | null,
    spec: null as Spec | null,
    error: null as string | null,
  });

  // 置顶状态
  let isPinned = $state(false);

  // 下拉框状态
  let showModeDropdown = $state(false);

  // 模式配置
  const modeConfig = $derived({
    show: { icon: Eye, label: t("selection.modeShow"), color: "text-error" },
    translate: {
      icon: Languages,
      label: t("selection.modeTranslate"),
      color: "text-info",
    },
    ai: { icon: Sparkles, label: t("selection.modeAi"), color: "text-primary" },
  });

  onMount(() => {
    console.log("=====> [selection/content] onMount executed");

    // 重置下拉框状态
    showModeDropdown = false;

    // 监听 init-content 事件
    const unlisten = listen("init-content", async (event: any) => {
      const { mode, text, x, y, app_info } = event.payload;
      content = { mode, text, app_info };
      // 新内容时重置置顶状态
      isPinned = false;
      await setContentPanelPinned(false);
      console.log("-----> content received: ", content);

      // 如果是翻译模式，自动开始翻译
      if (mode === "translate" && text) {
        await handleTranslate();
      }

      // // 设置位置：x 居中，y 在选中文字下方
      // await appWindow.setPosition(new LogicalPosition(x - 160, y + 8));
      // await appWindow.show();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  // 关闭面板
  async function handleClose() {
    content = {
      mode: "",
      text: "",
      app_info: { name: "", bundle_id: "", pid: 0 },
    };
    isPinned = false;
    await hideContentPanel();
  }

  // 切换置顶状态
  async function togglePin() {
    isPinned = !isPinned;
    await setContentPanelPinned(isPinned);
  }

  // 复制文本
  async function handleCopy() {
    await writeText(content.text);
  }

  // 重新生成
  async function handleRegenerate() {
    // TODO: 触发重新生成逻辑
    console.log("重新生成:", content.mode);
  }

  // 继续问
  async function handleContinue() {
    // TODO: 触发继续问逻辑
    console.log("继续问");
  }

  // 切换模式
  async function handleModeChange(newMode: "show" | "translate" | "ai") {
    content.mode = newMode;
    showModeDropdown = false;
    // TODO: 触发模式切换逻辑，重新生成内容
    console.log("模式切换为:", newMode);

    // 如果切换到翻译模式且有文本，自动开始翻译
    if (newMode === "translate" && content.text) {
      await handleTranslate();
    }
  }

  // 切换下拉框显示状态
  function toggleDropdown() {
    showModeDropdown = !showModeDropdown;
  }

  // 点击外部关闭下拉框
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".mode-dropdown")) {
      showModeDropdown = false;
    }
  }

  // 翻译会话的 system prompt：约束模型只回 JSON，与 parseTranslationResponse
  // 的字段契约一致（解析失败时按纯文本回落，不会硬失败）。
  const TRANSLATION_PROMPT =
    'You are a translation assistant. Translate the user\'s input between Chinese and English (auto-detect the source and translate to the other language). Reply with ONLY a JSON object and no other text: {"translation": "<translated text>", "targetLanguage": "<zh|en>", "phonetic": "<pronunciation, or null>", "explanation": "<brief usage note in Chinese, or null>"}';

  /**
   * 获取翻译 Session：优先使用设置里选定的翻译 Agent（quickTools.
   * translationAgentId），未选定时回落 builtin-chat + 硬编码翻译 prompt。
   *
   * 缓存的 sessionId 仅在「创建它的 agent（translation.agentId）」与当前配置
   * 一致时复用——用户在设置里切换翻译 Agent 后，这里自动按新 Agent 重建会话并
   * 写回 settings（本处是唯一创建点）。模型统一取 quick-action 默认模型
   * （Agent 定义已与模型解耦，实例化必须显式给 model）；无可用模型时返回
   * null，上层提示去设置配置。
   */
  async function getOrCreateTranslationSession(): Promise<string | null> {
    try {
      // 设置可能在主窗口被修改过（换了翻译 Agent / 默认模型）：强制刷新本窗口
      // 的 settings 快照，再决定复用还是重建。
      await settingsState.loadSettings(true);

      const configuredAgentId =
        settingsState.settings?.quickTools?.translationAgentId ?? null;
      const cached = settingsState.settings?.translation;
      if (cached?.sessionId && (cached.agentId ?? null) === configuredAgentId) {
        return cached.sessionId;
      }

      if (getAllModels().length === 0) {
        await providerActions.loadProvidersWithModels(false);
      }
      const resolved = resolveQuickActionModel(
        settingsState.settings?.quickAction,
        getAllModels(),
      );
      if (!resolved.available) {
        return null;
      }

      const session = await createSessionFromDefinition(
        configuredAgentId ?? "builtin-chat",
        {
          modelId: resolved.modelId,
          providerId: resolved.providerId,
        },
      );
      if (!configuredAgentId) {
        // builtin 回落：JSON 输出契约挂在会话 system prompt 上；失败不阻塞
        // （回落纯文本解析）。选定 Agent 时用其自带 system prompt / GenUI 配置。
        try {
          await updateAgentSessionField(
            session.id,
            "systemPrompt",
            TRANSLATION_PROMPT,
          );
        } catch (error) {
          console.warn("Failed to set translation system prompt:", error);
        }
      }
      await settingsState.updateSettings({
        section: "translation",
        data: { sessionId: session.id, agentId: configuredAgentId },
      });
      return session.id;
    } catch (error) {
      console.error("Failed to get translation session:", error);
      return null;
    }
  }

  /**
   * 解析翻译响应
   */
  function parseTranslationResponse(
    content: string,
    term: string,
  ): TranslationResult {
    try {
      const jsonMatch = content.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        const parsed = JSON.parse(jsonMatch[0]);
        return {
          term,
          translation: parsed.translation || content,
          targetLanguage: parsed.targetLanguage || "unknown",
          phonetic: parsed.phonetic || null,
          explanation: parsed.explanation || null,
        };
      }

      return {
        term,
        translation: content,
        targetLanguage: "unknown",
        phonetic: null,
        explanation: null,
      };
    } catch (error) {
      console.error("Failed to parse translation response:", error);
      return {
        term,
        translation: content,
        targetLanguage: "unknown",
        phonetic: null,
        explanation: null,
      };
    }
  }

  /**
   * 执行翻译
   */
  async function handleTranslate() {
    if (!content.text || translation.isLoading) return;

    const sessionId = await getOrCreateTranslationSession();
    if (!sessionId) {
      translation.error = t("selection.translationConfigHint");
      return;
    }

    translation.isLoading = true;
    translation.error = null;
    translation.result = null;
    translation.spec = null;

    const term = content.text;
    try {
      // 一问一答：纯文本增量实时回灌预览；spec 形状的流不逐字符渲染原始
      // JSON，保持加载态直到定稿。结束后先按 GenUI spec 解析，非 spec 回落
      // 结构化译文解析。
      const finalContent = await runAgentTextTurn(sessionId, term, (partial) => {
        if (looksLikeStreamingSpec(partial)) return;
        translation.result = {
          term,
          translation: partial,
          targetLanguage: "unknown",
          phonetic: null,
          explanation: null,
        };
      });
      const spec = resolveSpec(finalContent);
      if (spec) {
        translation.result = null;
        translation.spec = spec;
      } else {
        translation.result = parseTranslationResponse(finalContent, term);
      }
    } catch (error) {
      console.error("Translation error:", error);
      translation.error = t("selection.translationFailed");
      // 缓存会话可能已失效（被删除等）：清掉绑定让「重新翻译」重建会话。
      try {
        await settingsState.updateSettings({
          section: "translation",
          data: { sessionId: null, agentId: null },
        });
      } catch (clearError) {
        console.warn("Failed to clear stale translation session:", clearError);
      }
    } finally {
      translation.isLoading = false;
    }
  }
</script>

<svelte:window onclick={handleClickOutside} />

<div
  class="flex flex-col w-full h-full bg-[var(--bg-card)] rounded-xl shadow-lg border border-[var(--hairline)] overflow-hidden"
>
  <!-- 标题栏 -->
  {#if content.mode && modeConfig[content.mode]}
    {@const config = modeConfig[content.mode]}
    <div
      class="flex items-center justify-between px-3 py-2 border-b border-base-300 cursor-move"
      data-tauri-drag-region
    >
      <!-- 模式下拉框 -->
      <div class="mode-dropdown relative">
        <button
          class="flex items-center gap-1.5 px-2 py-1.5 rounded-lg hover:bg-base-300 transition-colors {config.color}"
          onclick={toggleDropdown}
        >
          <config.icon class="size-4" />
          <span class="text-sm font-medium">{config.label}</span>
          <ChevronDown class="size-3.5 opacity-60" />
        </button>

        <!-- 下拉菜单 -->
        {#if showModeDropdown}
          <div
            class="absolute top-full left-0 mt-1 bg-[var(--bg-card)] rounded-lg shadow-lg border border-[var(--hairline)] py-1 min-w-[120px] z-50"
          >
            {#each Object.entries(modeConfig) as [key, value]}
              {@const isActive = key === content.mode}
              <button
                class="flex items-center gap-2 w-full px-3 py-2 text-sm hover:bg-base-300 transition-colors {isActive
                  ? 'bg-base-300'
                  : ''}"
                class:text-primary={isActive}
                class:text-base-content={!isActive}
                onclick={() =>
                  handleModeChange(key as "show" | "translate" | "ai")}
              >
                <value.icon class="size-4" />
                <span>{value.label}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-300 transition-colors {isPinned
            ? 'text-primary'
            : 'text-base-content/50 hover:text-base-content'}"
          onclick={togglePin}
          title={isPinned ? t("selection.unpin") : t("selection.pin")}
        >
          {#if isPinned}
            <Pin class="size-3.5" />
          {:else}
            <PinOff class="size-3.5" />
          {/if}
        </button>
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-300 text-base-content/50 hover:text-base-content transition-colors"
          onclick={handleClose}
        >
          <X class="size-4" />
        </button>
      </div>
    </div>
  {/if}

  <!-- 内容区域 -->
  <!-- 划词结果是正文：译文 / 原文要能选中拷贝。 -->
  <div class="flex-1 p-3 overflow-auto min-h-0 select-text">
    {#if content.mode === "translate"}
      <!-- 翻译模式：流式期间纯文本增量走 result 预览，spec 形状的流保持加载态 -->
      {#if translation.isLoading && !translation.result}
        <div class="flex items-center justify-center py-8">
          <Spinner size={28} />
          <span class="ml-2 text-sm text-base-content/60">{t("selection.translating")}</span>
        </div>
      {:else if translation.error}
        <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
          {translation.error}
        </div>
      {:else if translation.spec}
        <!-- 翻译 Agent 的 GenUI 输出：整条回复是合法 JSON-Render spec → 卡片渲染 -->
        <JsonUIProvider initialState={{}}>
          <Renderer spec={translation.spec} registry={uiRegistry} />
        </JsonUIProvider>
      {:else if translation.result}
        <div class="space-y-3">
          <!-- 译文 -->
          <div class="p-2 rounded-lg bg-base-300">
            <div class="flex items-center gap-2">
              <span
                class="text-sm text-base-content whitespace-pre-wrap break-words font-medium"
              >
                {translation.result.translation}
              </span>
              {#if translation.result.phonetic}
                <span class="text-xs text-base-content/50">
                  [{translation.result.phonetic}]
                </span>
              {/if}
            </div>

            {#if translation.result.explanation}
              <p class="text-xs text-base-content/70 mt-1">
                {translation.result.explanation}
              </p>
            {/if}
          </div>
        </div>
      {:else}
        <p class="text-sm text-base-content/40 text-center py-4">{t("selection.waitingTranslation")}</p>
      {/if}
    {:else if content.text}
      <p
        class="text-sm text-base-content whitespace-pre-wrap break-words leading-relaxed"
      >
        {content.text}
      </p>
    {:else}
      <p class="text-sm text-base-content/40 text-center py-4">{t("selection.noContent")}</p>
    {/if}
  </div>

  <!-- 底部按钮区域 -->
  <div
    class="flex items-center justify-between px-3 py-1.5 border-t border-[var(--hairline)] bg-base-300/60"
  >
    <!-- 左下角：复制、重新生成 -->
    <div class="flex items-center gap-1">
      <button
        class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
        onclick={handleCopy}
        title={t("common.copy")}
        disabled={!content.text}
      >
        <Copy class="size-3.5" />
      </button>
      {#if content.mode === "translate"}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleTranslate}
          title={t("selection.retranslate")}
          disabled={!content.text || translation.isLoading}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {:else}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleRegenerate}
          title={t("selection.regenerate")}
          disabled={!content.text}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {/if}
    </div>

    <!-- 右下角：继续问 -->
    <button
      class="flex items-center px-2 py-1 text-xs font-medium text-primary hover:bg-primary/10 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
      onclick={handleContinue}
      title={t("selection.continueAsk")}
      disabled={!content.text}
    >
      <MessageCirclePlus class="size-3.5" />
      <span>{t("selection.continueAsk")}</span>
    </button>
  </div>
</div>
