<script lang="ts">
  import {
    ArrowUp,
    Square,
    Plus,
    X,
    FileText,
    FilePlus,
    FilePen,
    Terminal,
    Search,
    FileSearch,
    FolderTree,
  } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import SkillSlashPopover from "./SkillSlashPopover.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { getAllModels } from "$lib/states/provider.svelte";
  import { runAgentStream, steerAgentRun } from "$lib/api/agentSession";
  import { listSkills } from "$lib/api/skill";
  import type {
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
  const thinkingLevelOptions = [
    { value: "off", label: "关闭" },
    { value: "low", label: "低" },
    { value: "medium", label: "中" },
    { value: "high", label: "高" },
  ];

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

  const thinkingLevel = $derived(session.thinkingLevel ?? "off");

  // 内置工具开关（per-session）：勾选写入 session.enabledTools 并持久化；
  // 开关 id == coding-agent 注册名（read/write/edit/bash/grep/find/ls），
  // 后端 build_agent_session 按这些名做实际 gating。
  // 这 7 个工具都在工作目录内操作（含 bash），故全部 `requiresWorkingDir`：
  // 会话无 working_dir 时开关置灰禁用、点击无效、hover 给说明 title。
  const builtinTools: {
    id: string;
    label: string;
    icon: typeof FileText;
    requiresWorkingDir: boolean;
  }[] = [
    { id: "read", label: "读取文件", icon: FileText, requiresWorkingDir: true },
    { id: "write", label: "写入文件", icon: FilePlus, requiresWorkingDir: true },
    { id: "edit", label: "编辑文件", icon: FilePen, requiresWorkingDir: true },
    { id: "bash", label: "执行命令", icon: Terminal, requiresWorkingDir: true },
    { id: "grep", label: "搜索内容", icon: Search, requiresWorkingDir: true },
    { id: "find", label: "查找文件", icon: FileSearch, requiresWorkingDir: true },
    { id: "ls", label: "列目录", icon: FolderTree, requiresWorkingDir: true },
  ];

  const hasWorkingDir = $derived(!!session.workingDir);
  const enabledTools = $derived(session.enabledTools ?? []);

  function isToolEnabled(toolId: string): boolean {
    return enabledTools.includes(toolId);
  }

  function isToolDisabled(tool: (typeof builtinTools)[number]): boolean {
    return tool.requiresWorkingDir && !hasWorkingDir;
  }

  function toggleTool(tool: (typeof builtinTools)[number]) {
    if (isToolDisabled(tool)) return;
    const current = enabledTools;
    const next = current.includes(tool.id)
      ? current.filter((id) => id !== tool.id)
      : [...current, tool.id];
    agentSessionActions
      .updateField(session.id, "enabledTools", next)
      .catch((error) => {
        console.error("Failed to update agent session enabled tools:", error);
      });
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
  // 选中 → 追加可移除的 forced-skill chip（按 name 去重）并清掉 textarea 的 /query。
  //
  // forcedSkills：选中的强制 skill 列表（按 name 去重）。本 feature 仅维护与渲染；
  // 下个 feature（forced-chip-send-lifecycle）把这些 skill 的 name 接入 sendAgentRun。
  let forcedSkills = $state<SkillInfo[]>([]);

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

  function selectSkill(skill: SkillInfo) {
    // 按 name 去重：已存在则仅消费 query、不重复加 chip。
    if (!forcedSkills.some((s) => s.name === skill.name)) {
      forcedSkills = [...forcedSkills, skill];
    }
    clearSlashQuery();
    closeSlashPopover();
  }

  function removeForcedSkill(name: string) {
    forcedSkills = forcedSkills.filter((s) => s.name !== name);
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
      if (event.key === "ArrowDown") {
        // 端点有界、不动文本光标（preventDefault）。
        event.preventDefault();
        if (slashCandidates.length > 0) {
          slashHighlight = Math.min(
            effectiveHighlight + 1,
            slashCandidates.length - 1,
          );
        }
        return;
      }
      if (event.key === "ArrowUp") {
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
      modelPrompt = "部分图片超过 10MB 已跳过";
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
    // 注意：mid-run steer 仅支持纯文本，附件直接丢弃（不随 steer 发送）；forced
    // chip 同样不随 steer 发送、也不被清空——forced 是给下一个完整 run 的瞬时态，
    // 不该被 mid-run steering 误清，故此分支不触碰 forcedSkills。
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
          error instanceof Error ? error.message : "发送 steering 消息失败";
      }
      return;
    }

    // 空/纯空白输入且无附件为 no-op：不发起 run，不产生气泡（VAL-RUN-010）。
    if (!input.trim() && attachments.length === 0) return;

    // 无模型则提示并阻断（防御性；创建会话通常已含模型）（VAL-RUN-010）。
    if (!session.modelId || !session.providerId) {
      modelPrompt = "请先选择模型";
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
    // forced chip 是本回合（per-turn）瞬时态：dispatch 前快照 + 清空，与 input/
    // attachments 同构。成功则保持清空（下一 run 的 forced_skills 为空，
    // VAL-SLASH-021）；失败则在 catch 里与文本/附件一并原子回填，杜绝「文本回来了
    // 但强制丢了」的不一致（VAL-SLASH-024）。chip 文案=skill 名，slash 已被消费，
    // 故发给模型的 user message 正文绝不含字面 `/skillname`（VAL-SLASH-007）。
    const sentForcedSkills = forcedSkills;
    const forcedSkillNames = forcedSkills.map((s) => s.name);
    input = "";
    attachments = [];
    forcedSkills = [];
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
      // 启动失败：回填输入、附件与 forced chip，提示错误，便于重试。
      input = text;
      attachments = sentAttachments;
      forcedSkills = sentForcedSkills;
      adjustTextareaHeight();
      modelPrompt =
        error instanceof Error ? error.message : "启动 Agent 运行失败";
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

<div
  class="flex flex-col bg-base-300 rounded-lg border border-[var(--hairline)] mx-auto w-full max-w-[800px]"
>
  <!-- relative 容器锚定浮层；浮层向上弹（bottom-full）以免落屏外/被时间线裁切。 -->
  <div class="relative">
    {#if slashOpen}
      <div class="absolute bottom-full left-3 z-30 mb-1">
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
        ? "等待审批中，请在弹窗中允许或拒绝"
        : "在这里输入消息，按 Enter 发送"}
      onkeydown={handleKeydown}
      oninput={handleInput}
      oncompositionstart={handleCompositionStart}
      oncompositionend={handleCompositionEnd}
      rows="1"
      disabled={awaitingApproval}
      class="bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto disabled:cursor-not-allowed disabled:opacity-60"
    ></textarea>
  </div>

  {#if forcedSkills.length}
    <div class="px-4 pb-2 flex flex-wrap gap-2">
      {#each forcedSkills as skill (skill.name)}
        <span
          class="flex items-center gap-1 rounded-md border border-info/50 bg-info/10 px-2 py-1 text-xs text-info"
        >
          <span class="truncate max-w-[160px]">{skill.name}</span>
          <button
            type="button"
            class="rounded-full p-0.5 transition-colors hover:bg-info/20"
            title="移除 skill"
            aria-label={`移除 skill ${skill.name}`}
            onclick={() => removeForcedSkill(skill.name)}
          >
            <X size={12} />
          </button>
        </span>
      {/each}
    </div>
  {/if}

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
            title="移除图片"
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
      <span>等待工具审批，对话已暂停</span>
    </div>
  {/if}

  {#if modelPrompt}
    <div class="px-4 pb-1 text-xs text-warning">
      {modelPrompt}
    </div>
  {/if}

  <div class="flex flex-row items-center justify-between gap-3 px-4 pt-0 pb-2">
    <div class="flex flex-row flex-wrap items-center gap-2">
      <IconButton
        icon={Plus}
        ariaLabel="添加图片"
        title="上传图片"
        onclick={handleAddAttachment}
      />

      <!-- 内置工具开关（per-session enabledTools；FS 工具无 working_dir 时置灰）。 -->
      {#each builtinTools as tool (tool.id)}
        {@const ToolIcon = tool.icon}
        {@const active = isToolEnabled(tool.id)}
        {@const disabled = isToolDisabled(tool)}
        <button
          type="button"
          class={`flex items-center gap-1 px-2 py-1 rounded-md border text-xs transition-colors ${
            disabled
              ? "border-[var(--hairline)] text-base-content/30 cursor-not-allowed"
              : active
                ? "border-info/50 bg-info/10 text-info"
                : "border-[var(--hairline)] text-base-content/60 hover:bg-base-200"
          }`}
          aria-pressed={active}
          {disabled}
          title={disabled
            ? `${tool.label}（需设置工作目录）`
            : active
              ? `${tool.label}：已启用`
              : `${tool.label}：已禁用`}
          onclick={() => toggleTool(tool)}
        >
          <ToolIcon size={14} />
          <span>{tool.label}</span>
        </button>
      {/each}

    </div>
    <div class="flex flex-row items-center gap-3">
      <Select
        value={thinkingLevel}
        options={thinkingLevelOptions}
        size="sm"
        autoWidth
        onChange={handleThinkingChange}
      />
      <ChatModelSelectButton
        {selectedModel}
        onModelSelect={handleModelSelect}
      />
      {#if running}
        <CircleButton
          icon={Square}
          iconSize={16}
          size="w-8 h-8"
          ariaLabel="停止"
          onclick={handleStop}
        />
      {:else}
        <CircleButton
          icon={ArrowUp}
          iconSize={18}
          size="w-8 h-8"
          ariaLabel="发送"
          disabled={awaitingApproval}
          onclick={sendAgentRun}
        />
      {/if}
    </div>
  </div>
</div>
