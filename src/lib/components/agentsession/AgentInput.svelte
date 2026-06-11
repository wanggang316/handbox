<script lang="ts">
  import {
    ArrowUp,
    Square,
    Plus,
    X,
    FileText,
    FolderTree,
    Globe,
    BookOpen,
  } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
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

  // 内置工具开关（per-session）：勾选写入 session.enabledTools 并持久化
  // （VAL-TOOLS-005 UI half；后端按 enabledTools 做实际 gating）。
  // `requiresWorkingDir` 的 FS 工具在会话无 working_dir 时置灰禁用。
  const builtinTools: {
    id: string;
    label: string;
    icon: typeof FileText;
    requiresWorkingDir: boolean;
  }[] = [
    { id: "read_file", label: "读取文件", icon: FileText, requiresWorkingDir: true },
    {
      id: "list_directory",
      label: "列目录",
      icon: FolderTree,
      requiresWorkingDir: true,
    },
    { id: "web_fetch", label: "网页抓取", icon: Globe, requiresWorkingDir: false },
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

  // per-session skill 勾选：与内置工具开关并列、但写的是 enabledSkills 这条独立链
  // （VAL-UI-012）。可用 skill 由 `listSkills(session.workingDir)` 给出 —— 传 workingDir
  // 才能纳入 project scope；无 workingDir 时 project skill 直接缺席（非置灰，VAL-UI-010）。
  // 跨 scope 同名已由后端 shadow 去重为单项（VAL-UI-017）。
  let availableSkills = $state<SkillInfo[]>([]);

  // workingDir 变化即重载可用 skill（含/不含 project scope）。$effect 跟踪
  // session.workingDir；其 cleanup 在 effect 重跑前把上一次的 `cancelled` 置真，
  // 使旧 workingDir 的迟到响应成为 no-op、不覆盖新值（VAL-UI-009）。
  $effect(() => {
    const workingDir = session.workingDir;
    let cancelled = false;
    listSkills(workingDir)
      .then((skills) => {
        if (!cancelled) {
          availableSkills = skills;
        }
      })
      .catch((error) => {
        if (!cancelled) {
          console.error("Failed to list skills for agent session:", error);
          availableSkills = [];
        }
      });
    return () => {
      cancelled = true;
    };
  });

  const enabledSkills = $derived(session.enabledSkills ?? []);

  function isSkillEnabled(name: string): boolean {
    return enabledSkills.includes(name);
  }

  // 非乐观：await IPC 返回的整条 session 替换本地态后勾选才翻转；失败仅 console.error，
  // 本地态保持原值（VAL-UI-011）。写入 name 单份（VAL-UI-017）。
  function toggleSkill(skill: SkillInfo) {
    const current = enabledSkills;
    const next = current.includes(skill.name)
      ? current.filter((name) => name !== skill.name)
      : [...current, skill.name];
    agentSessionActions
      .updateField(session.id, "enabledSkills", next)
      .catch((error) => {
        console.error("Failed to update agent session enabled skills:", error);
      });
  }

  // 该会话是否存在活跃 run —— 驱动 Send <-> Stop 切换（VAL-RUN-006）。
  const running = $derived(agentRunStore.isRunning(session.id));

  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = "auto";
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 200;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + "px";
    }
  }

  // Enter 发送；Shift+Enter 换行（镜像 ChatInput）（VAL-RUN-011）。
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendAgentRun();
    }
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
    // run 进行中：消息走 steering 队列，不起第二个 run。后端 agent_run_steer 把
    // 文本压入活跃 run 的 steering 队列、在 turn 边界 drain；纯空白为干净 no-op。
    // 注意：mid-run steer 仅支持纯文本，附件直接丢弃（不随 steer 发送）。
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
    input = "";
    attachments = [];
    adjustTextareaHeight();
    try {
      await runAgentStream(session.id, text, payloadAttachments);
      // 发送成功后再 revoke 预览 URL（此时缩略图已从 DOM 移除）。
      sentAttachments.forEach((a) => {
        if (a.previewUrl.startsWith("blob:")) {
          URL.revokeObjectURL(a.previewUrl);
        }
      });
    } catch (error) {
      // 启动失败：回填输入与附件，提示错误，便于重试。
      input = text;
      attachments = sentAttachments;
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
  <textarea
    bind:this={textareaRef}
    bind:value={input}
    placeholder="在这里输入消息，按 Enter 发送"
    onkeydown={handleKeydown}
    oninput={adjustTextareaHeight}
    rows="1"
    class="bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto"
  ></textarea>

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

      <!-- per-session skill 勾选（enabledSkills 独立链；只渲染 listSkills 实际返回的
           skill，磁盘失踪的幽灵勾选 name 自然缺席、不呈静默坏勾选，VAL-UI-014；
           project skill 在无 workingDir 时不出现，VAL-UI-010）。 -->
      {#each availableSkills as skill (skill.name)}
        {@const active = isSkillEnabled(skill.name)}
        <button
          type="button"
          class={`flex items-center gap-1 px-2 py-1 rounded-md border text-xs transition-colors max-w-[160px] ${
            active
              ? "border-info/50 bg-info/10 text-info"
              : "border-[var(--hairline)] text-base-content/60 hover:bg-base-200"
          }`}
          aria-pressed={active}
          title={active
            ? `Skill ${skill.name}：已启用`
            : `Skill ${skill.name}：已禁用`}
          onclick={() => toggleSkill(skill)}
        >
          <BookOpen size={14} class="shrink-0" />
          <span class="truncate">{skill.name}</span>
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
          onclick={sendAgentRun}
        />
      {/if}
    </div>
  </div>
</div>
