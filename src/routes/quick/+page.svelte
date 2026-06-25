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
  - 审批 / 停止 / steering / run 错误 / new-clear 重置属于后续两个 feature
    （qa-overlay-approvals / qa-run-controls-errors）；本页仅保留对应 stub 回调。
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
  import { runAgentStream } from "$lib/api/agentSession";
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

  // ⌘↵ continue-in-chat 仅在有内容时可用（主窗口交接留待后续里程碑）。
  const canContinue = $derived(value.trim().length > 0);

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
   * - running 中：不发（按钮变 Stop，留待 F10）。
   * - 无会话：解析模型；空态展示配置引导、不建会话；否则建会话、置 sessionId、run。
   * - 有会话：直接对同一会话 run（follow-up，保留上下文）。
   */
  async function handleSend(text: string): Promise<void> {
    if (!text.trim()) return;
    // 待审批暂停：对话挂起在一次危险工具调用上，送出为干净 no-op，直到用户在
    // 审批弹窗里允许 / 拒绝（VAL-COMMS-016）。
    if (awaitingApproval) return;
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

    try {
      const session = await agentSessionActions.createSession(decision.request);
      sessionId = session.id;
      configurePrompt = null;
      await startRun(session.id, text);
    } catch (error) {
      console.error("quick: failed to create session", error);
    }
  }

  /** 启动一次 run；输入随即清空，流式输出经 AgentTimeline 渲染。 */
  async function startRun(id: UUID, text: string): Promise<void> {
    value = "";
    focusInput();
    try {
      await runAgentStream(id, text);
    } catch (error) {
      // run 级错误处理留待 qa-run-controls-errors（F10）；此处仅记录。
      console.error("quick: failed to start run", error);
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

  // ── 暂为 stub 的回调（留待后续 feature 实装）─────────────────────────────
  function handleContinue(): void {
    // continue-in-chat（主窗口交接）属于后续里程碑。
    console.log("quick:continue", value);
  }

  function handleStop(): void {
    // stop/abort 属于 qa-run-controls-errors（F10）。
    console.log("quick:stop");
  }

  function handleNewClear(): void {
    // new/clear 重置属于 qa-run-controls-errors（F10）；此处仅清空草稿。
    value = "";
    focusInput();
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
