<script lang="ts">
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import { ChevronsUpDown, Plus, ArrowUp, X, Pencil } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Button from "../ui/Button.svelte";
  import ChatModelSelectModal from "./ChatModelSelectModal.svelte";
  import { currentChatModel, chatActions } from "$lib/states/chat.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    messageInput?: string;
    editingMessageId?: string | null;
    onSendMessage?: (message: string) => void;
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
  let showModelModal = $state(false);

  // 直接使用 chatState 中的显示模型
  const currentModel = $derived(currentChatModel().model);

  // 判断是否处于编辑模式
  const isEditing = $derived(editingMessageId !== null && editingMessageId !== undefined);

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
    if (!messageInput.trim()) return;
    onSendMessage(messageInput);
    messageInput = "";
  }

  function handleCancelEdit() {
    if (onCancelEdit) {
      onCancelEdit();
    }
  }

  function handleAddAttachment() {
    console.log("添加附加");
  }
</script>

<div
  class="flex flex-col bg-base-200 rounded-xl border border-base-300 max-h-[300px] mx-auto w-full max-w-[800px]"
>
  <!-- 编辑模式提示 -->
  {#if isEditing}
    <div class="flex items-center justify-between px-4 pt-3 pb-2 border-b border-base-300">
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
    placeholder={isEditing ? "编辑消息内容..." : "在这里输入消息，按 Enter 发送"}
    onkeydown={handleKeydown}
    oninput={adjustTextareaHeight}
    rows="1"
    class="bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto"
  ></textarea>

  <div
    class="flex flex-row justify-between items-center px-4 pt-0 pb-2 overflow-visible"
  >
    <!-- 左侧：添加按钮 -->
    <IconButton
      icon={Plus}
      ariaLabel="添加附加"
      on:click={handleAddAttachment}
    />

    <!-- 右侧：模型选择和发送按钮 -->
    <div class="flex items-center gap-3">
      <Button
        variant="clear"
        size="sm"
        on:click={() => showModelModal = true}
        >
        {currentModel ? currentModel.name : "选择模型"}
        <ChevronsUpDown size={14} />
      </Button>
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

<!-- 模型选择模态框 -->
<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={currentModel}
  onModelSelect={(model) => {
    // 通过 chatActions 更新当前聊天的模型
    chatActions.updateChatModel(model.id, model.provider_id);
  }}
/>
