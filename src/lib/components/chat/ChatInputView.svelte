<script lang="ts">
  import CircleButton from "$lib/components/ui/CircleButton.svelte";
  import { ChevronsUpDown, Plus, Send } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";
  import Button from "../ui/Button.svelte";
  import ChatModelSelectModal from "./ChatModelSelectModal.svelte";
  import { chatState } from "$lib/states/chat.svelte";

  interface Props {
    messageInput?: string;
    onSendMessage?: (message: string) => void;
  }

  let {
    messageInput = $bindable(""),
    onSendMessage = (message: string) =>
      console.log("Sending message:", message),
  }: Props = $props();

  let textareaRef: HTMLTextAreaElement;
  let showModelModal = $state(false);

  // 从状态管理中获取当前选中的模型
  const selectedModel = $derived(chatState.selectedModel);

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
    }
  }

  function sendMessage() {
    if (!messageInput.trim()) return;
    onSendMessage(messageInput);
    messageInput = "";
  }

  function handleAddAttachment() {
    console.log("添加附加");
  }
</script>

<div
  class="flex flex-col bg-[#f7f7f7] rounded-xl border border-[#ebeaea] max-h-[300px] mx-auto w-full max-w-[800px]"
>
  <textarea
    bind:this={textareaRef}
    bind:value={messageInput}
    placeholder="在这里输入消息，按 Enter 发送"
    onkeydown={handleKeydown}
    oninput={adjustTextareaHeight}
    rows="1"
    class="bg-transparent text-[14px] text-[#7e7e7f] p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto"
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
        {selectedModel ? selectedModel.name : "选择模型"}
        <ChevronsUpDown size={14} />
      </Button>
      <CircleButton icon={Send} ariaLabel="发送" on:click={sendMessage} />
    </div>
  </div>
</div>

<!-- 模型选择模态框 -->
<ChatModelSelectModal
  bind:open={showModelModal}
/>
