<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import Modal from "../../ui/Modal.svelte";

  let draftPrompt = $state("");
  let showModal = $state(false);
  let modalRef = $state<Modal>();

  const hasActiveChat = $derived(Boolean(chatState.currentChat));
  const promptText = $derived(chatState.currentChat?.systemPrompt ?? "");
  const hasPrompt = $derived(promptText.trim().length > 0);

  function handleOpenModal() {
    if (!hasActiveChat) return;
    draftPrompt = chatState.currentChat?.systemPrompt ?? "";
    showModal = true;
  }

  function handleCancelModal() {
    modalRef?.handleClose();
  }

  async function handleSaveModal() {
    if (!hasActiveChat) {
      modalRef?.handleClose();
      return;
    }

    const remotePrompt = chatState.currentChat?.systemPrompt ?? "";
    if (draftPrompt === remotePrompt) {
      modalRef?.handleClose();
      return;
    }

    try {
      await chatActions.updateSystemPrompt(draftPrompt);
      modalRef?.handleClose();
    } catch (error) {
      console.error("Failed to update system prompt:", error);
      // 保持 Modal 打开以便用户重试
    }
  }

  function handleCloseModal() {
    // Modal 关闭时不做任何保存操作
    showModal = false;
  }
</script>

<button
  class="w-full space-y-4 rounded-2xl bg-base-200 px-5 py-4 hover:bg-base-300"
  type="button"
  onclick={handleOpenModal}
  disabled={!hasActiveChat}
>
  <div class="flex items-start justify-between gap-4">
    <div class="space-y-1">
      <h3 class="text-sm text-base-content">System Prompt</h3>
    </div>
  </div>

  <div
    class="px-0 py-0 text-sm text-left leading-relaxed text-base-content/70 line-clamp-3 overflow-hidden"
  >
    {#if hasPrompt}
      {promptText}
    {:else}
      <span class="text-base-content/50">暂无系统提示词</span>
    {/if}
  </div>
</button>

<Modal
  bind:this={modalRef}
  bind:open={showModal}
  title="编辑系统提示词"
  onClose={handleCloseModal}
>
  <div class="w-[70vw] h-[80vh] px-6 pt-16 pb-6 flex flex-col gap-5">
    <div class="flex-1 min-h-0">
      <textarea
        bind:value={draftPrompt}
        placeholder="输入系统提示词..."
        class="w-full h-full px-3 py-2 border border-base-300 rounded-md resize-none
               focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
               font-mono text-sm text-base-content bg-base-200
               scrollbar-thin scrollbar-thumb-base-300 scrollbar-track-base-200
               hover:scrollbar-thumb-base-300/80"
      ></textarea>
    </div>

    <div class="flex items-center justify-between">
      <div class="text-xs text-base-content/70">
        字符数: {draftPrompt.length}
      </div>
      <div class="flex items-center gap-3">
        <button
          type="button"
          class="rounded-full border border-base-300 px-4 py-2 text-sm font-medium text-base-content hover:border-base-300/70 hover:bg-base-200 transition-colors"
          onclick={handleCancelModal}
        >
          取消
        </button>
        <button
          type="button"
          class="rounded-full border border-base-300 px-4 py-2 text-sm font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors"
          onclick={handleSaveModal}
        >
          完成
        </button>
      </div>
    </div>
  </div>
</Modal>
