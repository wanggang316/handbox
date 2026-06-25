<!--
  Quick Action 浮层 composer 宿主页(基于 chat Agent 的两步式 Raycast 浮层)。

  一张铺满 frameless / transparent NSPanel 的圆角主题卡片,内含 QuickInput composer。
  NSPanel 隐藏窗口而非销毁 webview,故每次召唤(窗口重新获得焦点)都重置为全新空白
  状态并重新聚焦输入框——保证「一次召唤 = 一个全新一回合文档」。

  两步式交互(用户确认的 Raycast UX):
  1. 选择步:输入即过滤 chat Agent 列表(agentState.agents);↑↓ 移动高亮、↵ / 点击
     选中。无 Agent 时引导去应用创建。
  2. 消息步:选中 Agent 后输入框切到「给 <Agent> 发消息…」;↵ 发送 → 经
     `createSessionFromAgent` 建一个真实 chat 会话(已带 Agent 的模型),再用
     `messageStore.sendMessage` 走与主对话完全一致的流式管线渲染回复。Backspace(空
     输入)取消已选 Agent 回到选择步。
  3. answered 步:一回合已发送 → 输入禁用,仅展示 transcript;⌘↵「在对话中继续」把这
     个已持久化的会话交给主窗口(/chat?id=)继续。这是个一回合文档,要继续就打开 app。

  无模型选择(Agent 自带模型)、无 New(+)、无停止/续问。Esc 任意时刻关闭浮层。
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Bot } from "@lucide/svelte";
  import QuickInput from "$lib/components/quickaction/QuickInput.svelte";
  import QuickAgentList from "$lib/components/quickaction/QuickAgentList.svelte";
  import QuickTranscript from "$lib/components/quickaction/QuickTranscript.svelte";
  import type { Agent, UUID } from "$lib/types";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import { t } from "$lib/i18n";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { chatState } from "$lib/states/chat.svelte";
  import { messageStore } from "$lib/states/message.svelte";
  import { providerActions, getAllModels } from "$lib/states/provider.svelte";
  import { createSessionFromAgent, updateChatModel } from "$lib/api/chat";

  let composer = $state<QuickInput | null>(null);

  // composer 本地状态(父级拥有,回调消费)。
  let value = $state("");
  // 选择步的高亮项索引(指向 filteredAgents)。
  let highlightIndex = $state(0);
  // 已选 Agent;null 表示仍在选择步。
  let selectedAgent = $state<Agent | null>(null);
  // 已发送一回合的 chat 会话 id;null 表示尚未发送(选择步 / 消息步)。
  let chatId = $state<UUID | null>(null);
  // 发送在途闸:防止消息步重复 Enter 建出第二个会话。
  let sending = $state(false);
  // ⌘↵ 交接在途闸:幂等,双击 ⌘↵ 第二次早返回。
  let continuing = $state(false);
  // 发送失败的兜底提示(footer 渲染)。
  let runError = $state<string | null>(null);

  // 选择步:按搜索词过滤 Agent(大小写不敏感的子串匹配);消息步 / answered 步不使用。
  const filteredAgents = $derived.by(() => {
    const query = value.trim().toLowerCase();
    const all = agentState.agents;
    if (!query) return all;
    return all.filter((a) => a.name.toLowerCase().includes(query));
  });

  // 选择步是否应展示内容区:Agent 已加载出来,或加载结束(以便展示空态 / 无匹配)。
  // 加载中且尚无 Agent 时不展示,使召唤瞬间仅有一条干净的输入条。
  const showPickerContent = $derived(
    selectedAgent === null &&
      (agentState.agents.length > 0 || !agentState.isLoading),
  );
  const hasContent = $derived(chatId !== null || showPickerContent);

  // answered 步(已发送)输入禁用;⌘↵ 仅 answered 步可用。
  const isAnswered = $derived(chatId !== null);
  const canContinue = $derived(chatId !== null);

  const placeholder = $derived(
    selectedAgent
      ? t("quickaction.messagePlaceholder", { name: selectedAgent.name })
      : t("quickaction.searchPlaceholder"),
  );

  // 选择步:搜索词变化时高亮回到首项。
  $effect(() => {
    void value;
    if (selectedAgent === null) highlightIndex = 0;
  });

  function focusInput(): void {
    composer?.focus();
  }

  /** 重置为全新空白状态(每次召唤 = 一个一回合文档)。 */
  function resetOverlay(): void {
    selectedAgent = null;
    chatId = null;
    value = "";
    highlightIndex = 0;
    runError = null;
    sending = false;
  }

  /** 选择步:选中一个 Agent,切入消息步。 */
  function selectAgent(agent: Agent): void {
    selectedAgent = agent;
    value = "";
    runError = null;
    focusInput();
  }

  /** 消息步:取消已选 Agent,退回选择步。 */
  function deselectAgent(): void {
    selectedAgent = null;
    value = "";
    highlightIndex = 0;
    focusInput();
  }

  /** 选择步:在 filteredAgents 中循环移动高亮。 */
  function moveHighlight(delta: number): void {
    const len = filteredAgents.length;
    if (len === 0) return;
    highlightIndex = (highlightIndex + delta + len) % len;
  }

  /**
   * ↵ 主动作:按当前步骤分派——选择步选中高亮 Agent,消息步发送消息,answered 步
   * 输入已禁用故为干净 no-op。
   */
  function handleSubmit(): void {
    if (isAnswered) return;
    if (selectedAgent === null) {
      const agent = filteredAgents[highlightIndex];
      if (agent) selectAgent(agent);
      return;
    }
    void sendMessage(selectedAgent, value);
  }

  /**
   * 消息步发送:建一个真实 chat 会话(已带 Agent 的模型/系统提示词),再走
   * `messageStore.sendMessage` 的流式管线。设 chatState.currentChat 供该管线与
   * QuickTranscript 读取(浮层 webview 独立单例,不影响主窗口)。
   *
   * 发送失败:回退到消息步,回填文本以便重试,展示 runError。
   */
  async function sendMessage(agent: Agent, text: string): Promise<void> {
    if (!text.trim()) return;
    if (sending) return;
    const agentId = agent.id;
    if (!agentId) return;

    sending = true;
    runError = null;
    try {
      let chat = await createSessionFromAgent(agentId);

      // Agent 仅存 model 字符串,新建会话的 provider_id 为空。按 model id 在已启用
      // catalog 中解析出 provider 并持久化——既让本次发送可用,也让「在对话中继续」后
      // 主窗口(从磁盘重载会话)同样带着 provider 可直接发送。
      if (chat.id && chat.modelId && !chat.providerId) {
        if (getAllModels().length === 0) {
          await providerActions.loadProvidersWithModels();
        }
        const match = getAllModels().find((m) => m.id === chat.modelId);
        if (match) {
          chat = await updateChatModel(chat.id, match.id, match.provider_id);
        }
      }

      if (!chat.modelId || !chat.providerId) {
        // 解析不到可用 provider(model 已下架,或 Agent 存了无效 model)。
        runError = t("quickaction.model.unavailable");
        value = text;
        focusInput();
        return;
      }

      chatState.currentChat = chat;
      chatId = chat.id ?? null;
      value = "";
      await messageStore.sendMessage(text, []);
    } catch (error) {
      console.error("quick: failed to send message", error);
      runError =
        error instanceof Error ? error.message : t("quickaction.runFailed");
      // 回退到消息步,保留已选 Agent,回填文本重试。
      chatId = null;
      chatState.currentChat = null;
      value = text;
      focusInput();
    } finally {
      sending = false;
    }
  }

  /**
   * ⌘↵「在对话中继续」:把这个已持久化的会话交给主窗口。后端
   * `quick_action_continue_in_chat` 据 chatId 前置主窗口并广播 `quick-action-open-chat`,
   * 主窗口 `(app)/+layout.svelte` 监听后 goto `/chat?id=<chatId>`。随后隐藏浮层;
   * 下次召唤经 focus 监听重置为全新空白状态。
   */
  async function handleContinue(): Promise<void> {
    if (chatId === null) return;
    if (continuing) return;
    if (!isTauriEnvironment()) return;

    const id = chatId;
    continuing = true;
    try {
      await invoke("quick_action_continue_in_chat", { chatId: id });
      await invoke("quick_action_hide");
    } catch (error) {
      console.error("quick: failed to continue in chat", error);
    } finally {
      continuing = false;
    }
  }

  /** 隐藏浮层(仅 Tauri 环境可解析该命令)。 */
  async function hideOverlay(): Promise<void> {
    if (!isTauriEnvironment()) return;
    await invoke("quick_action_hide");
  }

  onMount(() => {
    focusInput();

    // 浮层是 (app) group 外的独立路由,不会跑主布局的 initialize;自行按需加载
    // Agent 列表(选择步数据源)与供应商(transcript 的模型图标)。
    agentActions.loadAgents().catch((error) => {
      console.error("quick: failed to load agents", error);
    });
    providerActions.loadProvidersWithModels().catch((error) => {
      console.error("quick: failed to load providers", error);
    });

    if (!isTauriEnvironment()) return;

    // webview 跨 hide/show 存活:后端在面板每次成为 key window(= 一次新召唤)时广播
    // `quick-action-shown`,据此重置为全新空白状态并重新聚焦,使「一次召唤 = 一个一回合
    // 文档」恒成立。nonactivating panel 的 onFocusChanged 在 hide/show 间不可靠,故改用
    // 原生 become-key 信号。
    let unlisten: UnlistenFn | null = null;
    let stale = false;
    listen("quick-action-shown", () => {
      resetOverlay();
      focusInput();
    })
      .then((fn) => {
        if (stale) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((error) => {
        console.error("quick: failed to listen for shown event", error);
      });

    return () => {
      stale = true;
      unlisten?.();
    };
  });

  /**
   * 窗口级键盘:Esc 任意时刻关闭浮层;⌘↵ 作为 answered 步(输入禁用,textarea 不再
   * 接收键盘)的兜底「在对话中继续」。选择/消息步的 ↵ / ↑↓ / Backspace 由 QuickInput
   * 的语义化回调驱动,不在此处理。
   */
  async function handleWindowKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key === "Escape") {
      event.preventDefault();
      await hideOverlay();
      return;
    }
    if (
      event.key === "Enter" &&
      (event.metaKey || event.ctrlKey) &&
      canContinue
    ) {
      event.preventDefault();
      await handleContinue();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<!--
  Raycast 式统一面板:输入行在顶,内容区(Agent 列表 / transcript / 空态)经 children
  注入,footer 在底。面板贴合内容高度,顶部对齐,余下窗口区域透明。
-->
<div class="flex h-full w-full flex-col overflow-hidden text-[var(--base-content)]">
  <QuickInput
    bind:this={composer}
    bind:value
    {placeholder}
    selectedAgentName={selectedAgent?.name ?? null}
    disabled={isAnswered}
    {canContinue}
    {runError}
    {hasContent}
    onSubmit={handleSubmit}
    onContinue={handleContinue}
    onArrowDown={() => moveHighlight(1)}
    onArrowUp={() => moveHighlight(-1)}
    onDeselect={deselectAgent}
  >
    {#snippet children()}
      {#if chatId !== null}
        <QuickTranscript {chatId} />
      {:else if selectedAgent === null}
        {#if agentState.agents.length === 0}
          <!-- 尚无 Agent:引导去应用创建。 -->
          <div class="flex flex-col items-center justify-center gap-3 px-6 py-9 text-center">
            <Bot size={26} class="text-[var(--base-content)]/35" />
            <div class="flex flex-col gap-1">
              <p class="text-sm font-medium">{t("quickaction.noAgents.title")}</p>
              <p class="text-xs text-[var(--base-content)]/55">
                {t("quickaction.noAgents.description")}
              </p>
            </div>
          </div>
        {:else if filteredAgents.length === 0}
          <!-- 有 Agent 但搜索无匹配。 -->
          <div class="px-4 py-6 text-center text-sm text-[var(--base-content)]/50">
            {t("quickaction.noMatch")}
          </div>
        {:else}
          <QuickAgentList
            agents={filteredAgents}
            {highlightIndex}
            onSelect={selectAgent}
            onHover={(i) => (highlightIndex = i)}
          />
        {/if}
      {/if}
    {/snippet}
  </QuickInput>
</div>

<style>
  /* 透明窗口:让 body 背景透出,仅卡片可见,保持 frameless 圆角浮层观感。 */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
