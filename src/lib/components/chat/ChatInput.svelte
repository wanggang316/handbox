<script lang="ts">
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import { Plus, ArrowUp, X, Pencil } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Button from "../ui/Button.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { currentChatModel, chatActions } from "$lib/states/chat.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import type { ChatAttachment } from "$lib/types/chat";
  import { onDestroy } from "svelte";

  interface Props {
    messageInput?: string;
    editingMessageId?: string | null;
    onSendMessage?: (message: string, attachments: ChatAttachment[]) => void;
    onCancelEdit?: () => void;
  }

  let {
    messageInput = $bindable(""),
    editingMessageId = $bindable(null),
    onSendMessage = (message: string) =>
      console.log("Sending message:", message),
    onCancelEdit,
  }: Props = $props();

  let textareaRef: HTMLTextAreaElement;
  let fileInputRef: HTMLInputElement | null = null;

  type AttachmentWithPreview = ChatAttachment & {
    id: string;
    size: number;
    previewUrl: string;
  };
  let attachments = $state<AttachmentWithPreview[]>([]);

  // 直接使用 chatState 中的显示模型
  const currentModel = $derived(currentChatModel().model);

  // 判断是否处于编辑模式
  const isEditing = $derived(
    editingMessageId !== null && editingMessageId !== undefined
  );

  // 检查当前模型是否支持非文本输入模态（如图片、PDF等）
  const supportsNonTextInput = $derived(
    currentModel?.input_modalities?.some((m) => m !== "text") ?? false
  );

  // 自动调整 textarea 高度
  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = "auto";
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 300;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + "px";
    }
  }

  // 监听 messageInput 变化，自动调整高度
  $effect(() => {
    if (messageInput !== undefined) {
      adjustTextareaHeight();
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    } else if (event.key === "Escape" && isEditing) {
      event.preventDefault();
      handleCancelEdit();
    }
  }

  function sendMessage() {
    if (!messageInput.trim() && attachments.length === 0) return;
    const payloadAttachments = attachments.map(({ name, mimeType, data }) => ({
      name,
      mimeType,
      data,
    }));
    onSendMessage(messageInput, payloadAttachments);
    resetAttachments();
    messageInput = "";
  }

  function handleCancelEdit() {
    if (onCancelEdit) {
      onCancelEdit();
    }
  }

  let showAttachmentMenu = $state(false);

  function handleAddAttachment(event?: MouseEvent) {
    event?.stopPropagation();
    if (isEditing) return;
    fileInputRef?.click();
  }

  async function handleAttachmentChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = input.files;
    if (!files || files.length === 0) {
      return;
    }

    const additions: AttachmentWithPreview[] = [];
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      const buffer = await file.arrayBuffer();
      additions.push({
        id: crypto.randomUUID(),
        name: file.name,
        mimeType: file.type || "image/png",
        data: new Uint8Array(buffer),
        size: file.size,
        previewUrl: URL.createObjectURL(file),
      });
    }

    if (additions.length) {
      attachments = [...attachments, ...additions];
    }

    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  function removeAttachment(id: string) {
    const target = attachments.find((item) => item.id === id);
    if (target?.previewUrl.startsWith("blob:")) {
      URL.revokeObjectURL(target.previewUrl);
    }
    attachments = attachments.filter((item) => item.id !== id);
  }

  function resetAttachments() {
    attachments.forEach((item) => {
      if (item.previewUrl.startsWith("blob:")) {
        URL.revokeObjectURL(item.previewUrl);
      }
    });
    attachments = [];
    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  $effect(() => {
    if (isEditing && attachments.length) {
      resetAttachments();
    }
  });

  onDestroy(() => {
    resetAttachments();
  });

  $effect(() => {
    if (!showAttachmentMenu) return;
    const handler = () => (showAttachmentMenu = false);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });
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
  class="flex flex-col bg-base-200 rounded-xl border border-base-300 max-h-[300px] mx-auto w-full max-w-[800px]"
>
  <!-- 编辑模式提示 -->
  {#if isEditing}
    <div
      class="flex items-center justify-between px-4 pt-3 pb-2 border-b border-base-300"
    >
      <div class="flex items-center gap-2 text-sm text-base-content/70">
        <Pencil size={14} />
        <span>编辑消息</span>
      </div>
      <button
        class="p-1 hover:bg-base-300 rounded transition-colors"
        title="取消编辑"
        onclick={handleCancelEdit}
      >
        <X size={16} />
      </button>
    </div>
  {/if}

  <textarea
    bind:this={textareaRef}
    bind:value={messageInput}
    placeholder={isEditing
      ? "编辑消息内容..."
      : "在这里输入消息，按 Enter 发送"}
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

  <div
    class="flex flex-row items-center px-4 pt-0 pb-2 overflow-visible"
    class:justify-between={supportsNonTextInput}
    class:justify-end={!supportsNonTextInput}
  >
    <!-- 左侧：添加按钮（仅当模型支持非文本输入时显示） -->
    {#if supportsNonTextInput}
      <IconButton
        icon={Plus}
        ariaLabel="添加附件"
        onclick={handleAddAttachment}
        title="上传图片"
        disabled={isEditing}
      />
    {/if}

    <!-- 右侧：模型选择和发送按钮 -->
    <div class="flex items-center gap-3">
      <ChatModelSelectButton
        selectedModel={currentModel}
        onModelSelect={(model) => {
          chatActions.updateChatModel(model.id, model.provider_id);
        }}
      />
      <CircleButton
        icon={ArrowUp}
        iconSize={18}
        size="w-8 h-8"
        ariaLabel={isEditing ? "更新消息" : "发送"}
        onclick={sendMessage}
      />
    </div>
  </div>
</div>
