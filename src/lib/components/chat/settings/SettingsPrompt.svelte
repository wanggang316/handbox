<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import Modal from "../../ui/Modal.svelte";
  import Textarea from "../../ui/Textarea.svelte";

  type SaveStatus = "saved" | "saving" | "error";

  let draftPrompt = $state(chatState.currentChat?.systemPrompt ?? "");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");
  let showModal = $state(false);

  const hasActiveChat = $derived(Boolean(chatState.currentChat));
  const promptText = $derived(chatState.currentChat?.systemPrompt ?? "");
  const hasPrompt = $derived(promptText.trim().length > 0);

  $effect(() => {
    // 同步外部 prompt 到本地草稿
    draftPrompt = chatState.currentChat?.systemPrompt ?? "";
    saveStatus = "saved";
  });

  $effect(() => {
    const remotePrompt = chatState.currentChat?.systemPrompt ?? "";
    if (draftPrompt === remotePrompt) {
      if (saveTimer) {
        clearTimeout(saveTimer);
        saveTimer = null;
      }
      return;
    }

    saveStatus = "saving";
    if (saveTimer) {
      clearTimeout(saveTimer);
    }

    saveTimer = setTimeout(async () => {
      saveTimer = null;
      await persistPrompt();
    }, 500);
  });

  function handleOpenModal() {
    if (!hasActiveChat) return;
    draftPrompt = chatState.currentChat?.systemPrompt ?? "";
    showModal = true;
  }

  async function persistPrompt() {
    if (!hasActiveChat) {
      saveStatus = "saved";
      return;
    }

    const remotePrompt = chatState.currentChat?.systemPrompt ?? "";
    if (draftPrompt === remotePrompt) {
      saveStatus = "saved";
      return;
    }

    try {
      await chatActions.updateSystemPrompt(draftPrompt);
      saveStatus = "saved";
    } catch (error) {
      console.error("Failed to update system prompt:", error);
      saveStatus = "error";
    }
  }

  async function handleCloseModal() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
      saveStatus = "saving";
      await persistPrompt();
    } else {
      await persistPrompt();
    }
    showModal = false;
  }
</script>

<button
  class="w-full space-y-4 rounded-2xl bg-base-100 px-5 py-4 hover:bg-base-300"
  type="button"
  onclick={handleOpenModal}
  disabled={!hasActiveChat}
>
  <div class="flex items-start justify-between gap-4">
    <div class="space-y-1">
      <h3 class="text-sm font-semibold text-base-content">System Prompt</h3>
    </div>
    <!-- <button
      type="button"
      class="rounded-full border border-base-300 px-4 py-2 text-sm font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors disabled:cursor-not-allowed disabled:opacity-60"
      onclick={handleOpenModal}
      disabled={!hasActiveChat}
    >
      编辑
    </button> -->
  </div>

  <div
    class="rounded-xl border border-dashed border-base-200 bg-base-200/40 px-2 py-2 text-sm text-left leading-relaxed text-base-content/70 whitespace-pre-wrap max-h-40 overflow-y-auto"
  >
    {#if hasPrompt}
      {promptText}
    {:else}
      <span class="text-base-content/50">暂无系统提示词</span>
    {/if}
  </div>

  {#if saveStatus !== "saved"}
    <div
      class="flex items-center justify-end gap-2 text-xs text-base-content/70"
    >
      <span
        class="h-2 w-2 rounded-full {saveStatus === 'saving'
          ? 'bg-warning'
          : 'bg-error'}"
      ></span>
      <span>{saveStatus === "saving" ? "保存中..." : "保存失败"}</span>
    </div>
  {/if}
</button>

<Modal bind:open={showModal} title="编辑系统提示词" onClose={handleCloseModal}>
  <div class="w-[520px] max-w-[90vw] px-6 pt-16 pb-4">
    <div class="space-y-5">
      <Textarea bind:value={draftPrompt} rows={12} showCharCount={true} />

      {#if saveStatus === "error"}
        <p class="text-xs text-error">保存失败，请检查网络连接后重试。</p>
      {/if}

      <div class="flex items-center justify-end gap-3 pt-2">
        {#if saveStatus === "saving"}
          <span class="text-xs text-base-content/60">保存中…</span>
        {/if}
        <button
          type="button"
          class="rounded-full border border-base-300 px-4 py-2 text-sm font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors"
          onclick={handleCloseModal}
        >
          完成
        </button>
      </div>
    </div>
  </div>
</Modal>
