<script lang="ts">
  import {
    ArrowUp,
    Square,
    Paperclip,
    X,
    Bot,
    ChevronDown,
    ChevronsUpDown,
    Check,
    Folder,
  } from "@lucide/svelte";
  import { onDestroy, tick } from "svelte";
  import { fly } from "svelte/transition";
  import { goto } from "$app/navigation";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import ModelSelectModal from "./ModelSelectModal.svelte";
  import SkillSlashPopover from "./SkillSlashPopover.svelte";
  import { t } from "$lib/i18n";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { getAllModels, getProviderIconById } from "$lib/states/provider.svelte";
  import { runAgentStream, steerAgentRun } from "$lib/api/agentSession";
  import { listSkills } from "$lib/api/skill";
  import type {
    Agent,
    AgentSession,
    AgentRunAttachment,
    SkillInfo,
  } from "$lib/types";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    session: AgentSession;
  }

  let { session }: Props = $props();

  // 思考强度档位（thinkingLevel 为后端自由文本字段）。
  // $derived so labels re-render on language switch.
  const thinkingLevelOptions = $derived([
    { value: "off", label: t("agent.thinking.off") },
    { value: "low", label: t("agent.thinking.low") },
    { value: "medium", label: t("agent.thinking.medium") },
    { value: "high", label: t("agent.thinking.high") },
  ]);

  // 单张图片软上限（10 MiB）。超限的图片不阻塞 UI，仅静默跳过并提示，避免把
  // 巨大的字节数组塞进 IPC 导致界面卡死/挂起（VAL-RUN-018）。
  const MAX_IMAGE_BYTES = 10 * 1024 * 1024;

  let input = $state("");
  let textareaRef: HTMLTextAreaElement;
  let modelPrompt = $state<string | null>(null);

  // 选中的图片附件（仅 image/*）。`previewUrl` 用 object URL 渲染缩略图，
  // 移除/发送/卸载时统一 revoke 以免内存泄漏。
  type AttachmentWithPreview = {
    id: string;
    name: string;
    mimeType: string;
    data: Uint8Array;
    previewUrl: string;
  };
  let attachments = $state<AttachmentWithPreview[]>([]);
  let fileInputRef: HTMLInputElement | null = null;

  // 会话存的是 modelId/providerId；picker 需要 ModelWithProvider，故从目录反查。
  const selectedModel = $derived<ModelWithProvider | null>(
    session.modelId && session.providerId
      ? (getAllModels().find(
          (m) =>
            m.id === session.modelId && m.provider_id === session.providerId,
        ) ?? null)
      : null,
  );
  const selectedModelIcon = $derived(
    selectedModel ? getProviderIconById(selectedModel.provider_id) : undefined,
  );
  // 模型选择 Modal（系统既有的搜索/收藏/分组模型选择器）开合。
  let modelModalOpen = $state(false);

  const thinkingLevel = $derived(session.thinkingLevel ?? "off");

  // ── Agent 选择器（把 Agents 页的「使用」搬进输入框左下角）──────────────────
  //    显示当前会话来源 Agent 名；点开向上弹出 AgentDefinition 列表，选中他者即
  //    从该定义实例化一个新会话并跳转过去（等价于在 Agents 页点「使用」）。选中
  //    当前会话自身来源的 Agent 为干净 no-op —— 不重复新建空会话。
  let agentMenuOpen = $state(false);

  // 当前会话来源 Agent（据 agentDefinitionId 从已加载列表反查；未加载/已删除/
  // 无 provenance 时为 null，按钮回落到「选择 Agent」占位）。
  const currentAgent = $derived<Agent | null>(
    session.agentDefinitionId
      ? (agentState.agents.find((a) => a.id === session.agentDefinitionId) ??
          null)
      : null,
  );
  const currentAgentLabel = $derived(
    currentAgent?.name ?? t("agent.input.selectAgent"),
  );

  // 是否显示「工作目录」选择：仅当会话来源 Agent 的 workingDirMode ≠ "none"
  // （required / optional / 旧定义 NULL 均需要工作目录，只有纯对话 "none" 不需要）。
  const showWorkingDir = $derived(
    !!currentAgent && currentAgent.workingDirMode !== "none",
  );
  // 工作目录展示名：取路径末段（basename）便于在紧凑按钮里显示；未设置为 null。
  const workingDirName = $derived.by(() => {
    const dir = session.workingDir;
    if (!dir) return null;
    const parts = dir.split("/").filter(Boolean);
    return parts.length > 0 ? parts[parts.length - 1] : dir;
  });

  // lazy-load Agent 列表（/agent 路由本身不加载它；两个内置 Agent 恒被 seed，故
  // length===0 即「尚未加载」的可靠代理）。打开选择器时、或会话已有来源 Agent 时
  // （后者用于解析 workingDirMode 以决定是否显示工作目录选择）都触发加载。
  $effect(() => {
    if (
      (agentMenuOpen || session.agentDefinitionId) &&
      agentState.agents.length === 0
    ) {
      agentActions
        .loadAgents()
        .catch((error) => console.error("Failed to load agents:", error));
    }
  });

  // 点击外部关闭（镜像工具菜单）。菜单内点击经 stopPropagation 不冒泡到 window。
  $effect(() => {
    if (!agentMenuOpen) return;
    const handler = () => (agentMenuOpen = false);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });

  function toggleAgentMenu(event: MouseEvent) {
    event.stopPropagation();
    agentMenuOpen = !agentMenuOpen;
  }

  async function selectAgent(agent: Agent) {
    agentMenuOpen = false;
    if (!agent.id) return;
    // 已是当前会话来源 Agent：干净 no-op（不重复新建空会话）。
    if (agent.id === session.agentDefinitionId) return;
    try {
      // 当前会话「一句话都没说过」（无消息且无活跃 run）：就地把它重指到新
      // Agent —— 复用现有会话，不新建（保留 id / URL，无需跳转）。否则从新定义
      // 实例化一个新会话并跳转过去。不传 overrides：后端让新定义的 model/工作目录
      // 策略优先，未定处再保留会话现值。
      if (session.messageCount === 0 && !agentRunStore.isRunning(session.id)) {
        await agentSessionActions.reinstantiateFromDefinition(
          session.id,
          agent.id,
        );
        return;
      }
      const created =
        await agentSessionActions.createSessionFromDefinition(agent.id);
      await goto(`/agent?id=${created.id}`);
    } catch (error) {
      console.error("Failed to switch agent:", error);
      modelPrompt = t("agent.input.switchAgentFailed");
    }
  }

  // 选择 / 更换会话工作目录：打开系统目录选择对话框，选中即持久化到 session.workingDir
  // （后端校验为已存在的绝对目录）。用户取消（返回非字符串）为干净 no-op。
  async function pickWorkingDir() {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const picked = await openDialog({ directory: true });
      if (typeof picked !== "string") return;
      modelPrompt = null;
      await agentSessionActions.updateField(session.id, "workingDir", picked);
    } catch (error) {
      console.error("Failed to set working directory:", error);
      modelPrompt =
        error instanceof Error
          ? error.message
          : t("agent.input.workingDirFailed");
    }
  }

  // 该会话是否存在活跃 run —— 驱动 Send <-> Stop 切换（VAL-RUN-006）。
  const running = $derived(agentRunStore.isRunning(session.id));

  // 该会话是否有待审批的危险工具调用（write/edit/bash）。待决期间对话暂停：
  // 输入框禁用、发送被拦截、提示「等待审批」，直到用户在弹窗里允许 / 拒绝
  // （VAL-CAPERM-001）。审批本身在页面级 AgentApprovalModal 里完成。
  const awaitingApproval = $derived(agentApprovalStore.hasPending(session.id));

  // ── Slash skill 自动补全浮层 ──────────────────────────────────────────
  // 触发条件：空输入框（整段 textarea 为空）首字符**键入** `/`（非粘贴、非词中、
  // 非 Shift+Enter 后行首、非 IME 合成）→ 打开锚定 textarea 的 skill 浮层。
  // 候选只含未禁用 skill；query 为 `/` 之后的文本，大小写不敏感子串匹配 name。
  // 选中 → 把 `/<name> ` 写回输入文本的原位置（替换已键入的 /query），不再单独用
  // chip 行展示。文本即唯一真源：发送时从行首 `/<name>` 解析出强制 skill 名传给后端，
  // 由后端把 skill body 注入 system prompt（forcedSkills wire 机制保持不变）。

  let slashOpen = $state(false);
  let slashQuery = $state("");
  let slashHighlight = $state(0);
  let availableSkills = $state<SkillInfo[]>([]);
  // composing 标记：IME 合成期间不触发浮层、不选中、不发送（VAL-SLASH-014）。
  let composing = $state(false);

  // 候选：未禁用 skill 经大小写不敏感 name 子串过滤（query 为空 → 全部）。
  const slashCandidates = $derived.by(() => {
    const q = slashQuery.trim().toLowerCase();
    const enabled = availableSkills.filter((s) => !s.disabled);
    if (!q) return enabled;
    return enabled.filter((s) => s.name.toLowerCase().includes(q));
  });

  // 高亮越界（过滤后列表缩短）时回钳到末项；空列表时无高亮（-1）。
  const effectiveHighlight = $derived(
    slashCandidates.length === 0
      ? -1
      : Math.min(slashHighlight, slashCandidates.length - 1),
  );

  async function loadAvailableSkills() {
    try {
      availableSkills = await listSkills(session.workingDir ?? undefined);
    } catch (error) {
      console.error("Failed to list skills for slash popover:", error);
      availableSkills = [];
    }
  }

  function openSlashPopover() {
    slashOpen = true;
    slashHighlight = 0;
    slashQuery = "";
    void loadAvailableSkills();
  }

  function closeSlashPopover() {
    slashOpen = false;
    slashQuery = "";
    slashHighlight = 0;
  }

  // 清掉 textarea 里从触发用 `/` 起的 query 文本（选中 / Escape / 退格关闭后）。
  function clearSlashQuery() {
    input = "";
    adjustTextareaHeight();
  }

  async function selectSkill(skill: SkillInfo) {
    // 把 `/<name> ` 写回输入文本的原位置（替换整段 /query），不再单独加 chip。
    // 关浮层后把焦点与光标交还 textarea 末尾，便于直接接着输入正文。强制 skill
    // 名在发送时从行首 `/<name>` 解析得到（见 leadingForcedSkillNames）。
    input = `/${skill.name} `;
    closeSlashPopover();
    await tick();
    if (!textareaRef) return;
    textareaRef.focus();
    const end = textareaRef.value.length;
    textareaRef.setSelectionRange(end, end);
    adjustTextareaHeight();
  }

  // 行首 `/<name>` → 强制 skill 名（仅匹配已知未禁用 skill；否则视为普通文本，
  // 返回空）。slash 触发只在整段恰为单个 `/` 时发生，故选中后正文总以 `/<name> `
  // 起头，至多一个前导强制 skill。
  function leadingForcedSkillNames(text: string): string[] {
    const m = text.trimStart().match(/^\/(\S+)/);
    if (!m) return [];
    const name = m[1];
    return availableSkills.some((s) => !s.disabled && s.name === name)
      ? [name]
      : [];
  }

  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = "auto";
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 200;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + "px";
    }
  }

  // Enter 发送；Shift+Enter 换行（镜像 ChatInput）（VAL-RUN-011）。
  // 浮层打开时优先消费键盘：↑/↓ 移高亮、Enter 选中、Escape 关闭——Enter 在浮层
  // 打开时绝不发送（VAL-SLASH-016）。IME 合成期间 Enter 不选不发（VAL-SLASH-014）。
  function handleKeydown(event: KeyboardEvent) {
    // IME 合成中：所有键交给输入法，不触发选中/发送（双保险：标记 + isComposing）。
    if (composing || event.isComposing) return;

    if (slashOpen) {
      // ↓ 或 Ctrl|Cmd+N 下移、↑ 或 Ctrl|Cmd+P 上移（emacs 风）；端点有界、
      // 不动文本光标（preventDefault）。高亮变化由浮层 scrollIntoView 跟随。
      const mod = event.metaKey || event.ctrlKey;
      const navDown =
        event.key === "ArrowDown" || (mod && event.key.toLowerCase() === "n");
      const navUp =
        event.key === "ArrowUp" || (mod && event.key.toLowerCase() === "p");
      if (navDown) {
        event.preventDefault();
        if (slashCandidates.length > 0) {
          slashHighlight = Math.min(
            effectiveHighlight + 1,
            slashCandidates.length - 1,
          );
        }
        return;
      }
      if (navUp) {
        event.preventDefault();
        if (slashCandidates.length > 0) {
          slashHighlight = Math.max(effectiveHighlight - 1, 0);
        }
        return;
      }
      if (event.key === "Enter" && !event.shiftKey) {
        // 浮层打开时 Enter = 选中而非发送；无高亮时干净 no-op。
        event.preventDefault();
        const target = slashCandidates[effectiveHighlight];
        if (target) selectSkill(target);
        return;
      }
      if (event.key === "Escape") {
        // 关闭并消费 /query。
        event.preventDefault();
        clearSlashQuery();
        closeSlashPopover();
        return;
      }
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendAgentRun();
    }
  }

  function handleCompositionStart() {
    composing = true;
  }

  function handleCompositionEnd() {
    composing = false;
    // 合成提交后正常字符流参与触发/过滤（VAL-SLASH-014）。
    syncSlashState(false);
  }

  // textarea 输入变化驱动浮层触发与 query 同步。`fromPaste` 时不开浮层
  // （粘贴的 `/` 不当触发，VAL-SLASH-012）。
  function syncSlashState(fromPaste: boolean) {
    // 合成中不触发（字符尚未提交）。
    if (composing) return;

    // 触发：整段输入恰为单个 `/` 且非粘贴 → 开浮层。
    if (!slashOpen) {
      if (!fromPaste && input === "/") openSlashPopover();
      return;
    }

    // 已打开：query = `/` 之后的文本。退格删掉触发用 `/`（input 不再以 `/` 开头
    // 或已空）→ 关闭浮层（VAL-SLASH-011）。
    if (!input.startsWith("/")) {
      closeSlashPopover();
      return;
    }
    slashQuery = input.slice(1);
    slashHighlight = 0;
  }

  function handleInput(event: Event) {
    adjustTextareaHeight();
    const inputType = (event as InputEvent).inputType;
    const fromPaste =
      inputType === "insertFromPaste" || inputType === "insertFromDrop";
    syncSlashState(fromPaste);
  }

  function handleAddAttachment(event?: MouseEvent) {
    event?.stopPropagation();
    fileInputRef?.click();
  }

  // 选图：仅接受 image/*，超限图片静默跳过（不阻塞 UI）。读成原始字节用于发送，
  // object URL 用于缩略图预览。
  async function handleAttachmentChange(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const files = target.files;
    if (!files || files.length === 0) return;

    const additions: AttachmentWithPreview[] = [];
    let skippedOversize = false;
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      if (file.size > MAX_IMAGE_BYTES) {
        skippedOversize = true;
        continue;
      }
      const buffer = await file.arrayBuffer();
      additions.push({
        id: crypto.randomUUID(),
        name: file.name,
        mimeType: file.type || "image/png",
        data: new Uint8Array(buffer),
        previewUrl: URL.createObjectURL(file),
      });
    }

    if (additions.length) {
      attachments = [...attachments, ...additions];
    }
    if (skippedOversize) {
      modelPrompt = t("agent.input.oversizeSkipped");
    }

    // 复位，使重选同一文件也能再次触发 change。
    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  function removeAttachment(id: string) {
    const item = attachments.find((a) => a.id === id);
    if (item?.previewUrl.startsWith("blob:")) {
      URL.revokeObjectURL(item.previewUrl);
    }
    attachments = attachments.filter((a) => a.id !== id);
  }

  function resetAttachments() {
    attachments.forEach((a) => {
      if (a.previewUrl.startsWith("blob:")) {
        URL.revokeObjectURL(a.previewUrl);
      }
    });
    attachments = [];
    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  onDestroy(() => {
    resetAttachments();
  });

  async function sendAgentRun() {
    // 待审批暂停：对话挂起在一次危险工具调用上，既不起新 run 也不入 steering 队列，
    // 直到用户在审批弹窗里允许 / 拒绝（VAL-CAPERM-001）。干净 no-op，不清空输入。
    if (awaitingApproval) return;

    // run 进行中：消息走 steering 队列，不起第二个 run。后端 agent_run_steer 把
    // 文本压入活跃 run 的 steering 队列、在 turn 边界 drain；纯空白为干净 no-op。
    // 注意：mid-run steer 仅支持纯文本，附件直接丢弃（不随 steer 发送）；正文里的
    // 行首 `/<name>` 不做强制 skill 解析（steer 不注入 skill），原样作为文本入队。
    // 活跃 run 必有模型，故此分支无需查 model 守卫；放在 model 守卫之前自洽。
    if (running) {
      // 纯空白输入：干净 no-op（不清空、不入队、不调用）。
      if (!input.trim()) return;
      modelPrompt = null;
      const text = input;
      resetAttachments();
      input = "";
      adjustTextareaHeight();
      try {
        await steerAgentRun(session.id, text);
      } catch (error) {
        // steer 失败：仅提示，不回填覆盖已清空的 input（保持简单）。
        console.error("Failed to steer agent run:", error);
        modelPrompt =
          error instanceof Error
            ? error.message
            : t("agent.input.steerFailed");
      }
      return;
    }

    // 空/纯空白输入且无附件为 no-op：不发起 run，不产生气泡（VAL-RUN-010）。
    if (!input.trim() && attachments.length === 0) return;

    // 无模型则提示并阻断（防御性；创建会话通常已含模型）（VAL-RUN-010）。
    if (!session.modelId || !session.providerId) {
      modelPrompt = t("agent.input.selectModelFirst");
      return;
    }

    modelPrompt = null;
    const text = input;
    // 快照附件用于发送（Uint8Array -> number[] 以匹配后端 Vec<u8> 的 IPC 形态），
    // 随即清空输入与附件；用户气泡由后端 emit 的 user message_end 经 agentRunStore
    // reduce 出现，此处不做乐观插入以免重复。
    const payloadAttachments: AgentRunAttachment[] = attachments.map((a) => ({
      name: a.name,
      mimeType: a.mimeType,
      data: Array.from(a.data),
    }));
    const sentAttachments = attachments;
    // 强制 skill 名从正文行首 `/<name>` 解析：文本即唯一真源，故 catch 回填 input
    // 即自动恢复强制 skill，无需单独快照。`/<name>` 随正文一并发给模型（与就地内联
    // 展示一致），同时后端按此 list 把 skill body 注入 system prompt。
    const forcedSkillNames = leadingForcedSkillNames(text);
    input = "";
    attachments = [];
    adjustTextareaHeight();
    try {
      await runAgentStream(
        session.id,
        text,
        payloadAttachments,
        forcedSkillNames,
      );
      // 发送成功后再 revoke 预览 URL（此时缩略图已从 DOM 移除）。
      sentAttachments.forEach((a) => {
        if (a.previewUrl.startsWith("blob:")) {
          URL.revokeObjectURL(a.previewUrl);
        }
      });
    } catch (error) {
      // 启动失败：回填输入与附件（input 含行首 `/<name>`，强制 skill 自动恢复），
      // 提示错误，便于重试。
      input = text;
      attachments = sentAttachments;
      adjustTextareaHeight();
      modelPrompt =
        error instanceof Error ? error.message : t("agent.input.runFailed");
    }
  }

  async function handleStop() {
    try {
      await agentRunStore.abort(session.id);
    } catch (error) {
      console.error("Failed to abort agent run:", error);
    }
  }

  function handleModelSelect(model: ModelWithProvider) {
    modelPrompt = null;
    agentSessionActions
      .updateField(session.id, "modelId", model.id)
      .then(() =>
        agentSessionActions.updateField(
          session.id,
          "providerId",
          model.provider_id,
        ),
      )
      .catch((error) => {
        console.error("Failed to update agent session model:", error);
      });
  }

  function handleThinkingChange(value: string) {
    agentSessionActions
      .updateField(session.id, "thinkingLevel", value)
      .catch((error) => {
        console.error("Failed to update agent session thinking level:", error);
      });
  }
</script>

<input
  type="file"
  accept="image/*"
  multiple
  class="hidden"
  bind:this={fileInputRef}
  onchange={handleAttachmentChange}
/>

<!-- 系统既有的模型选择 Modal（搜索/收藏/分组）。选中即成对写入 modelId+providerId。 -->
<ModelSelectModal
  bind:open={modelModalOpen}
  {selectedModel}
  onModelSelect={handleModelSelect}
/>

<!-- 工作目录选择：置于 composer 上方左侧，仅当来源 Agent 的 workingDirMode ≠ "none"
     时出现。点击打开系统目录选择框，选中即持久化到 session.workingDir。 -->
{#if showWorkingDir}
  <div class="mx-auto flex w-full max-w-[800px] pb-1">
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-base-content/55 hover:bg-base-300/50 hover:text-base-content transition-colors"
      aria-label={t("agent.input.selectWorkingDir")}
      title={session.workingDir ?? t("agent.input.selectWorkingDir")}
      onclick={pickWorkingDir}
    >
      <Folder size={13} class="shrink-0" />
      {#if workingDirName}
        <span class="max-w-[220px] truncate">{workingDirName}</span>
      {:else}
        <span class="max-w-[220px] truncate text-warning"
          >{t("agent.input.selectWorkingDir")}</span
        >
      {/if}
    </button>
  </div>
{/if}

<div
  class="flex flex-col bg-[var(--bg-page)] rounded-lg border border-[var(--hairline)] mx-auto w-full max-w-[800px]"
>
  <!-- relative 容器锚定浮层；浮层向上弹（bottom-full）以免落屏外/被时间线裁切。 -->
  <div class="relative">
    {#if slashOpen}
      <!-- fly 提供与 Agent 菜单一致的轻微位移 + 淡入淡出开合动画。 -->
      <div
        class="absolute bottom-full left-3 z-30 mb-1"
        transition:fly={{ y: -4, duration: 130 }}
      >
        <SkillSlashPopover
          items={slashCandidates}
          highlightedIndex={effectiveHighlight}
          onSelect={selectSkill}
          onHover={(index) => (slashHighlight = index)}
        />
      </div>
    {/if}
    <textarea
      bind:this={textareaRef}
      bind:value={input}
      placeholder={awaitingApproval
        ? t("agent.input.awaitingApprovalPlaceholder")
        : t("agent.input.placeholder")}
      onkeydown={handleKeydown}
      oninput={handleInput}
      oncompositionstart={handleCompositionStart}
      oncompositionend={handleCompositionEnd}
      rows="1"
      disabled={awaitingApproval}
      class="composer-input bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto disabled:cursor-not-allowed disabled:opacity-60"
    ></textarea>
  </div>

  {#if attachments.length}
    <div class="px-4 pb-2 flex flex-wrap gap-3">
      {#each attachments as attachment (attachment.id)}
        <div
          class="relative w-20 h-20 rounded-lg overflow-hidden border border-base-300 bg-base-100"
        >
          <img
            src={attachment.previewUrl}
            alt={attachment.name}
            class="w-full h-full object-cover"
          />
          <button
            class="absolute top-1 right-1 p-1 bg-base-200/80 hover:bg-base-200 rounded-full text-base-content transition-colors"
            type="button"
            title={t("agent.input.removeImage")}
            onclick={() => removeAttachment(attachment.id)}
          >
            <X size={12} />
          </button>
        </div>
      {/each}
    </div>
  {/if}

  {#if awaitingApproval}
    <!-- 待审批暂停指示：对话挂起在一次危险工具调用上，等待弹窗中的允许 / 拒绝
         （VAL-CAPERM-001）。 -->
    <div class="px-4 pb-1 flex items-center gap-2 text-xs text-warning">
      <span
        class="h-2 w-2 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
      ></span>
      <span>{t("agent.input.awaitingApprovalHint")}</span>
    </div>
  {/if}

  {#if modelPrompt}
    <div class="px-4 pb-1 text-xs text-warning">
      {modelPrompt}
    </div>
  {/if}

  <div class="flex flex-row items-center justify-between gap-3 px-4 pt-0 pb-2">
    <div class="flex flex-row flex-wrap items-center gap-2">
      <!-- 附件（图片上传）：最左侧的附件图标，与其余触发器共用安静 hover。 -->
      <button
        type="button"
        class="flex h-7 w-7 items-center justify-center rounded-md text-base-content transition-colors hover:bg-base-300/60"
        aria-label={t("agent.input.addImage")}
        title={t("agent.input.uploadImage")}
        onclick={handleAddAttachment}
      >
        <Paperclip size={16} />
      </button>

      <!-- Agent 选择器：当前会话来源 Agent + 向上弹出的切换列表。选中他者即从该
           AgentDefinition 实例化新会话并跳转（把 Agents 页的「使用」搬进输入框）。 -->
      <div class="relative">
        <button
          type="button"
          class={`flex h-7 items-center gap-1.5 rounded-md pl-1.5 pr-2 transition-colors ${
            agentMenuOpen
              ? "bg-base-300/60 text-base-content"
              : "text-base-content hover:bg-base-300/60"
          }`}
          aria-label={t("agent.input.selectAgent")}
          aria-haspopup="menu"
          aria-expanded={agentMenuOpen}
          title={t("agent.input.selectAgent")}
          onclick={toggleAgentMenu}
        >
          <Bot size={16} class="shrink-0" />
          <span class="max-w-[140px] truncate text-sm">{currentAgentLabel}</span>
          <ChevronDown size={14} class="shrink-0 opacity-60" />
        </button>

        {#if agentMenuOpen}
          <!-- 向上展开（bottom-full）：输入框在底部，列表浮于按钮上方以免落屏外。
               fly 提供轻微位移 + 淡入淡出的开合动画。
               stopPropagation 防止菜单内点击冒泡到 window 触发外部关闭。 -->
          <div
            transition:fly={{ y: -4, duration: 130 }}
            class="absolute bottom-full left-0 z-40 mb-2 max-h-72 w-64 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
            role="menu"
            tabindex="-1"
            onclick={(event) => event.stopPropagation()}
            onkeydown={() => {}}
          >
            {#if agentState.agents.length === 0}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                {t("common.loading")}
              </div>
            {:else}
              {#each agentState.agents as agent (agent.id)}
                {@const active = agent.id === session.agentDefinitionId}
                <button
                  type="button"
                  role="menuitem"
                  class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                    active ? "bg-base-300/60" : ""
                  }`}
                  onclick={() => selectAgent(agent)}
                >
                  <Bot size={16} class="shrink-0 text-base-content/70" />
                  <span
                    class="min-w-0 flex-1 truncate text-sm text-base-content"
                  >
                    {agent.name}
                  </span>
                  {#if active}
                    <Check size={14} class="shrink-0 text-primary" />
                  {/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    </div>
    <div class="flex flex-row items-center gap-3">
      <!-- 会话级模型选择器：打开系统既有的模型选择 Modal（搜索/收藏/分组）。选中即
           成对写入 modelId+providerId（handleModelSelect）；解析不到显示「选择模型」。 -->
      <button
        type="button"
        class="flex h-7 items-center gap-1.5 rounded-md px-2 py-1 text-sm text-base-content/80 hover:bg-base-300/60 transition-colors"
        aria-label={t("agent.input.selectModel")}
        title={selectedModel?.name ?? t("agent.input.selectModel")}
        onclick={() => (modelModalOpen = true)}
      >
        {#if selectedModel}
          {#if selectedModelIcon}
            <img
              src={selectedModelIcon}
              alt={selectedModel.providerName}
              class="h-4 w-4 shrink-0 rounded object-contain"
            />
          {/if}
          <span class="max-w-[160px] truncate">{selectedModel.name}</span>
        {:else}
          <span class="max-w-[160px] truncate text-warning"
            >{t("agent.input.selectModel")}</span
          >
        {/if}
        <ChevronsUpDown size={13} class="shrink-0 opacity-60" />
      </button>

      <!-- 推理等级：原生 select 套安静触发器样式，与模型触发器同高、同 hover。 -->
      <div class="relative">
        <select
          value={thinkingLevel}
          onchange={(event) => handleThinkingChange(event.currentTarget.value)}
          class="h-7 cursor-pointer appearance-none rounded-md bg-transparent pl-2 pr-6 py-1 text-sm text-base-content/80 hover:bg-base-300/60 transition-colors"
        >
          {#each thinkingLevelOptions as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <ChevronsUpDown
          size={13}
          class="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 text-base-content/80 opacity-60"
        />
      </div>
      {#if running}
        <CircleButton
          icon={Square}
          iconSize={16}
          size="w-8 h-8"
          customClass="enabled:hover:opacity-90"
          ariaLabel={t("agent.input.stop")}
          onclick={handleStop}
        />
      {:else}
        <CircleButton
          icon={ArrowUp}
          iconSize={18}
          size="w-8 h-8"
          customClass="enabled:hover:opacity-90"
          ariaLabel={t("agent.input.send")}
          disabled={awaitingApproval}
          onclick={sendAgentRun}
        />
      {/if}
    </div>
  </div>
</div>
