<script lang="ts">
  import { ArrowUp, Square, Plus, X } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { getAllModels } from "$lib/states/provider.svelte";
  import { runAgentStream } from "$lib/api/agentSession";
  import type { AgentSession, AgentRunAttachment } from "$lib/types";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    session: AgentSession;
  }

  let { session }: Props = $props();

  // 思考强度档位（与 AgentSessionCreateModal 保持一致；thinkingLevel 为后端自由文本字段）。
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
    <IconButton
      icon={Plus}
      ariaLabel="添加图片"
      title="上传图片"
      onclick={handleAddAttachment}
    />
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
