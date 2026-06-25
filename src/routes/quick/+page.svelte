<!--
  Quick Action 浮层 composer 宿主页。

  一张铺满 frameless / transparent NSPanel 的圆角主题卡片，内含 QuickInput
  composer。Esc 调用 `quick_action_hide` 隐藏浮层。

  关键：NSPanel 隐藏窗口而非销毁 webview，故除 onMount 外，还需在窗口重新获得焦点
  时再次聚焦输入框，确保每次召唤都能立即键入（VAL-OVERLAY-007）。

  会话语义（qa-session-send-stream / F8）：
  - 本页是 `sessionId` 状态的唯一真源（null 直到首次发送）。后续 feature
    （continue-in-chat、new/clear）都将操纵这同一个状态。
  - 首次非空发送：守空态 → 解析模型（空态则展示配置引导、不建会话）→ 懒建一个
    一次性 AgentSession（无 projectId → sandbox cwd）→ `runAgentStream`。
  - 后续发送（上一轮已结束）复用同一个会话（保留上下文），不新建。
  - 流式渲染复用 `AgentTimeline` + `agentRunStore`（按 sessionId 分键）。
  - 停止 / steering / run 错误 / new-clear 重置（qa-run-controls-errors）：
    · Stop → `agentRunStore.abort(sessionId)`（VAL-COMMS-012）。
    · running 中发送 → `steerAgentRun`，由 in-flight reply 消费，不起第二个 run
      （VAL-COMMS-004/024）。
    · run-START 失败 → 回填输入 + 展示错误（VAL-COMMS-018）；mid-STREAM 错误经
      `runState.error` 由 AgentTimeline 渲染（VAL-COMMS-009）。
    · new/clear → sessionId=null + 清空，重置为全新空白会话（VAL-COMMS-011）。
    · hide（Esc/blur/hotkey）绝不 abort，run 跨 hide 存活（VAL-COMMS-008）。
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import QuickInput from "$lib/components/quickaction/QuickInput.svelte";
  import AgentTimeline from "$lib/components/agentsession/AgentTimeline.svelte";
  import AgentApprovalModal from "$lib/components/agentsession/AgentApprovalModal.svelte";
  import { Settings } from "@lucide/svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import type { UUID } from "$lib/types";
  import type {
    AgentApprovalRequest,
    ApprovalDecision,
  } from "$lib/types/agentSession";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import { t } from "$lib/i18n";
  import { getAllModels } from "$lib/states/provider.svelte";
  import { settingsState } from "$lib/states/settings.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { runAgentStream, steerAgentRun } from "$lib/api/agentSession";
  import { openSettingsWindow } from "$lib/api/window";
  import {
    resolveQuickActionModel,
    type QuickActionModelResolution,
    type QuickActionEmptyReason,
  } from "$lib/quickaction/resolveModel";
  import { buildQuickSessionRequest } from "$lib/quickaction/createSessionRequest";

  let composer = $state<QuickInput | null>(null);

  // composer 本地状态（父级拥有，回调消费）。
  let value = $state("");
  let selectedModel = $state<ModelWithProvider | null>(null);

  // 会话状态（本页唯一真源）：null 直到首次成功发送后懒建。
  let sessionId = $state<UUID | null>(null);
  // 无可用模型时的配置引导空态（首次发送解析为空时置位；选定模型即清除）。
  let configurePrompt = $state<QuickActionEmptyReason | null>(null);
  // run 启动失败的错误提示（runAgentStream 同步拒绝时置位；下一次发送/输入清除）。
  // mid-STREAM 错误由 AgentTimeline 渲染 runState.error，不经本状态（VAL-COMMS-009）。
  let runError = $state<string | null>(null);

  // 首次发送的 in-flight 闸：handleSend 是 async，冷召唤时同步守卫后 await
  // createSession，sessionId 仅在 await 之后赋值。无此闸，await 期间第二次 Enter
  // 会重入同一分支（sessionId 仍 null、running 仍 false）建出第二个一次性会话 +
  // run，首个 run 成孤儿计费。故在 await 前同步置位、finally 清除，重入即早返回。
  let creating = $state(false);

  // running 反映该会话是否有活跃 run；驱动 Send <-> Stop（停止留待 F10）。
  // 无会话时恒为 false。
  const running = $derived(
    sessionId !== null && agentRunStore.isRunning(sessionId),
  );

  // 当前会话的待审批请求（危险工具调用 write/edit/bash → 弹审批弹窗、对话暂停）。
  // 镜像 /agent 页：按本浮层的 sessionId 分键；后端 app-wide 广播，弹窗在挂载了
  // 该 sessionId 的上下文里弹出（VAL-COMMS-005）。
  const pendingApproval = $derived(
    sessionId !== null ? agentApprovalStore.pendingFor(sessionId) : null,
  );

  // 待审批期间输入暂停（送出为干净 no-op，VAL-COMMS-016）。
  const awaitingApproval = $derived(pendingApproval !== null);

  // ⌘↵ continue-in-chat 仅在已有会话（首次发送后）时可用：把这个一次性会话交接给
  // 主窗口。发送后 composer 通常已清空，故以「有无会话」而非「有无文本」为闸——
  // 首发前 / new-clear 后无会话，⌘↵ 是干净 no-op（VAL-CONTINUE-005/006）。
  const canContinue = $derived(sessionId !== null);

  // continue-in-chat 交接的 in-flight 闸：handleContinue 是 async（invoke +
  // hide），双击 ⌘↵ 时第二次须早返回，避免重复 invoke / 重复 navigate
  // （VAL-CONTINUE-007 幂等）。交接完成后会话指针即重置，亦自然挡住重入。
  let continuing = $state(false);

  /** 聚焦输入框（下一帧，确保窗口/DOM 就绪后才聚焦）。 */
  function focusInput(): void {
    composer?.focus();
  }

  /**
   * 解析本回合应使用的模型：in-panel 选择优先，否则回退到 quick-action 默认。
   *
   * in-panel 选中的是已在 catalog 内的 `ModelWithProvider`（snake_case
   * `provider_id`），直接构造 available 结果；未选时交给 `resolveQuickActionModel`
   * 用 settings 默认 + catalog 匹配（含空态原因）。
   */
  function resolveModel(): QuickActionModelResolution {
    if (selectedModel) {
      return {
        available: true,
        modelId: selectedModel.id,
        providerId: selectedModel.provider_id,
        model: selectedModel,
      };
    }
    return resolveQuickActionModel(
      settingsState.settings?.quickAction,
      getAllModels(),
    );
  }

  /**
   * 发送：首次发送守空态 → 懒建会话 → run；后续发送复用同一会话。
   *
   * - 空 / 纯空白：干净 no-op（QuickInput 已先一道闸，此处再兜底）。
   * - running 中：文本走 steering 队列注入活跃 run，不起第二个 run（VAL-COMMS-004/024）。
   * - 无会话：解析模型；空态展示配置引导、不建会话；否则建会话、置 sessionId、run。
   * - 有会话：直接对同一会话 run（follow-up，保留上下文）。
   */
  async function handleSend(text: string): Promise<void> {
    if (!text.trim()) return;
    // 首次发送的 in-flight 重入守卫：createSession 进行中再次 Enter 直接早返回，
    // 避免冷召唤时建出第二个一次性会话 + 孤儿 run（见 `creating` 声明处）。
    if (creating) return;
    // 待审批暂停：对话挂起在一次危险工具调用上，送出为干净 no-op，直到用户在
    // 审批弹窗里允许 / 拒绝（VAL-COMMS-016）。
    if (awaitingApproval) return;

    // run 进行中：文本走 steering 队列注入活跃 run，不起第二个 run（不触发
    // AGENT_RUN_ALREADY_ACTIVE）。镜像 AgentInput.sendAgentRun：清空输入后再调
    // agent_run_steer，由 in-flight reply 在 turn 边界消费（VAL-COMMS-004/024）。
    // steer 失败仅提示、不回填已清空的输入（与 AgentInput 一致，丢弃该 follow-up）。
    if (running && sessionId !== null) {
      const id = sessionId;
      value = "";
      focusInput();
      try {
        await steerAgentRun(id, text);
      } catch (error) {
        console.error("quick: failed to steer agent run", error);
      }
      return;
    }
    // 防御：running 但 sessionId 已不存在（理论上不发生，running 依赖 sessionId）。
    if (running) return;

    // Follow-up：会话已存在，复用之，不新建。
    if (sessionId !== null) {
      await startRun(sessionId, text);
      return;
    }

    // 首次发送：解析模型并懒建会话。
    const decision = buildQuickSessionRequest({
      resolution: resolveModel(),
      defaultEnabledTools: settingsState.settings?.agent?.defaultEnabledTools,
      name: t("quickaction.sessionName"),
    });

    if (decision.status === "empty") {
      // 无可用模型：展示配置引导，绝不建出不可运行的会话。
      configurePrompt = decision.reason;
      return;
    }

    // 同步置闸（在 await 之前），冷召唤期间的第二次 Enter 命中开头的 `creating`
    // 早返回；finally 清闸，无论建会话成败下一次发送都能正常进入。
    creating = true;
    try {
      const session = await agentSessionActions.createSession(decision.request);
      sessionId = session.id;
      configurePrompt = null;
      await startRun(session.id, text);
    } catch (error) {
      console.error("quick: failed to create session", error);
      // 建会话失败：浮层否则一片沉寂、看似冻结。置 runError（QuickInput 已渲染该
      // 状态）让用户看见失败；已键入文本仍在 value 中，可直接重试。
      runError =
        error instanceof Error ? error.message : t("quickaction.runFailed");
    } finally {
      creating = false;
    }
  }

  /**
   * 启动一次 run；输入随即清空，流式输出经 AgentTimeline 渲染。
   *
   * run-START 同步失败（runAgentStream 拒绝）：回填已清空的输入并展示错误，便于
   * 重试（镜像 AgentInput ~453-461，VAL-COMMS-018）。mid-STREAM 错误不走此路径——
   * 它经 agent_stream_error 落入 runState.error，由 AgentTimeline 渲染（VAL-COMMS-009）。
   */
  async function startRun(id: UUID, text: string): Promise<void> {
    runError = null;
    value = "";
    focusInput();
    try {
      await runAgentStream(id, text);
    } catch (error) {
      console.error("quick: failed to start run", error);
      // 启动失败：回填输入、展示错误，便于重试。
      value = text;
      focusInput();
      runError =
        error instanceof Error ? error.message : t("quickaction.runFailed");
    }
  }

  /**
   * 回应审批弹窗当前展示的请求（含作用域）：deny → 工具被 Cancel、对话继续；
   * allow_once → 本次允许；allow_always → 本会话始终允许该工具（后端按 sessionId
   * 键控的进程内存集，VAL-COMMS-005/025）。透传**弹窗持有的 request 引用**，store
   * 据其 requestId 精确回灌（展示==回灌，无重取竞态）后清键关弹窗。Esc 关闭由弹窗
   * 内部 fail-closed 映射为 deny（VAL-COMMS-017），此处只处理用户主动决策。
   */
  function handleApprovalRespond(
    request: AgentApprovalRequest,
    decision: ApprovalDecision,
  ): void {
    void agentApprovalStore.respondTo(request, decision);
  }

  /** 打开设置窗口（模型/供应商配置页），让用户配置默认模型后再回来。 */
  async function openModelSettings(): Promise<void> {
    try {
      await openSettingsWindow("/models");
    } catch (error) {
      console.error("quick: failed to open settings", error);
    }
  }

  /**
   * ⌘↵ continue-in-chat：把当前一次性会话交接给主窗口（VAL-CONTINUE-001..012）。
   *
   * 浮层与主窗口是两个独立 webview JS 上下文，交接不传递任何内存态——只传 sessionId。
   * 后端 `quick_action_continue_in_chat` 据 id 把主窗口前置并广播 `quick-action-open-session`
   * （payload = 裸 session-id 字符串），主窗口 `(app)/+layout.svelte` 监听后 goto
   * `/agent?id=<id>`，从磁盘 + app-wide 事件广播重建「相同 transcript」（live run 续跑、
   * 审批在主窗口里解析一次）。
   *
   * - 无会话（首发前 / new-clear 后）：干净 no-op，绝不建空 /agent、绝不前置主窗口
   *   （VAL-CONTINUE-005/006）。canContinue 已先一道闸，此处兜底。
   * - 双击 ⌘↵：`continuing` 闸 + 交接后立即重置会话指针 → 单次 navigate、不双聚焦
   *   （VAL-CONTINUE-007 幂等）。
   * - 交接后重置会话指针（sessionId=null + 清空输入/错误/配置引导），使下一次召唤是
   *   全新空 composer，不向后泄漏状态（VAL-CONTINUE-011）。复用 handleNewClear 的
   *   重置路径以与 new/clear 语义一致；但绝不 abort——交接的 run 须在主窗口续活。
   */
  async function handleContinue(): Promise<void> {
    if (sessionId === null) return;
    if (continuing) return;
    if (!isTauriEnvironment()) return;

    const id = sessionId;
    continuing = true;
    try {
      await invoke("quick_action_continue_in_chat", { sessionId: id });
      await invoke("quick_action_hide");
      // 交接成功：重置为全新空白会话（不 abort——run 已属主窗口，跨 hide 续活）。
      resetSession();
    } catch (error) {
      console.error("quick: failed to continue in chat", error);
    } finally {
      continuing = false;
    }
  }

  /** Stop：中止当前会话的活跃 run（finalized/aborted turn，输入重新可用，VAL-COMMS-012）。 */
  function handleStop(): void {
    if (sessionId === null) return;
    void agentRunStore.abort(sessionId).catch((error) => {
      console.error("quick: failed to abort agent run", error);
    });
  }

  /**
   * 重置为一个全新的空白一次性会话——sessionId→null、清空输入 / run 错误 / 配置引导；
   * timeline 随 sessionId 消失，直到下一次发送懒建全新会话。
   *
   * 由 new/clear（先 abort 旧 run）与 continue-in-chat（交接后、绝不 abort）共用，
   * 二者「重置为全新空会话」的语义一致（VAL-COMMS-011 / VAL-CONTINUE-011）。abort
   * 与否的差异由各调用方在调用前自行决定，此处只做纯粹的状态清零。
   */
  function resetSession(): void {
    sessionId = null;
    value = "";
    runError = null;
    configurePrompt = null;
    focusInput();
  }

  /**
   * new/clear：彻底重置为一个全新的空白一次性会话（VAL-COMMS-011）。
   *
   * 若仍有活跃 run，先 abort 旧 run，避免一个无可见界面的孤儿 run 继续在后台跑；
   * 随后复用 resetSession 清空状态。
   */
  function handleNewClear(): void {
    if (sessionId !== null && running) {
      void agentRunStore.abort(sessionId).catch((error) => {
        console.error("quick: failed to abort agent run on new/clear", error);
      });
    }
    resetSession();
  }

  function handleModelSelect(model: ModelWithProvider): void {
    selectedModel = model;
    // 选定模型即解除配置引导空态。
    configurePrompt = null;
  }

  onMount(() => {
    // 首次挂载即聚焦（首次召唤）。
    focusInput();

    if (!isTauriEnvironment()) {
      // 纯浏览器预览：无 Tauri 窗口事件，挂载聚焦已足够验证渲染/聚焦/键入。
      return;
    }

    // webview 跨 hide/show 存活：每次窗口重新获得焦点都重新聚焦，
    // 使 VAL-OVERLAY-007 在每次召唤（而非仅首次）都成立。
    let unlisten: UnlistenFn | null = null;
    getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) focusInput();
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => unlisten?.();
  });

  /**
   * Esc → 隐藏浮层（仅在 Tauri 环境可解析该命令）。
   *
   * 待审批期间让位给审批弹窗：弹窗自身把 Esc 接到 fail-closed deny（VAL-COMMS-017），
   * 此时不隐藏整张浮层——否则一次 Esc 既隐藏窗口又决策，UX 混乱且偏离 /agent。
   * 弹窗的 Esc 在其 backdrop 上处理后冒泡到此，故在此守门、不再 hide。
   */
  async function handleKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key !== "Escape") return;
    if (awaitingApproval) return;
    event.preventDefault();
    if (!isTauriEnvironment()) return;
    await invoke("quick_action_hide");
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="flex h-full w-full flex-col gap-2 overflow-hidden text-[var(--base-content)]"
>
  {#if sessionId !== null}
    <!-- 有会话：可滚动 timeline 占据余下空间，composer 固定在底部卡片内。 -->
    <div
      class="min-h-0 flex-1 overflow-y-auto rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] shadow-2xl"
    >
      <AgentTimeline {sessionId} />
    </div>
  {:else if configurePrompt !== null}
    <!-- 无可用模型：配置引导空态（不建会话，提供前往设置的入口）。 -->
    <div
      class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-xl border border-[var(--hairline)] bg-[var(--bg-card)] p-6 text-center shadow-2xl"
    >
      <Settings size={28} class="text-[var(--base-content)]/40" />
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium">{t("quickaction.noModel.title")}</p>
        <p class="text-xs text-[var(--base-content)]/60">
          {t("quickaction.noModel.description")}
        </p>
      </div>
      <button
        type="button"
        onclick={openModelSettings}
        class="rounded-lg border border-[var(--hairline)] bg-[var(--bg-base)] px-3 py-1.5 text-xs font-medium hover:bg-[var(--bg-base)]/80"
      >
        {t("quickaction.noModel.openSettings")}
      </button>
    </div>
  {/if}

  <!--
    composer slot：有会话时 timeline 占满余下空间，composer 按内容高度收缩
    （`shrink-0` + 上限），保持卡片观感；无会话时它独占整张卡片。
  -->
  <div class="shrink-0 {sessionId !== null ? 'max-h-[240px]' : 'flex-1'}">
    <QuickInput
      bind:this={composer}
      bind:value
      {running}
      {awaitingApproval}
      {runError}
      {selectedModel}
      {canContinue}
      onModelSelect={handleModelSelect}
      onSend={handleSend}
      onContinue={handleContinue}
      onStop={handleStop}
      onNewClear={handleNewClear}
    />
  </div>

  <!-- 工具审批弹窗：危险工具调用（write/edit/bash）待决期间弹出、对话暂停；
       allow_once / allow_always / deny 经 store 回灌后端后关闭。Esc 关闭由弹窗内部
       fail-closed 映射为 deny（VAL-COMMS-005/017/025）。按本浮层会话分键。 -->
  {#if pendingApproval}
    <AgentApprovalModal
      request={pendingApproval}
      onRespond={handleApprovalRespond}
    />
  {/if}
</div>

<style>
  /* 透明窗口：让 body 背景透出，仅卡片可见，保持 frameless 圆角浮层观感。 */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
