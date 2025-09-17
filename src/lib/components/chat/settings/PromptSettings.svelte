<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { TableGroup, TextareaRow } from "../../ui/table";

  type SaveStatus = "saved" | "saving" | "error";

  let currentPrompt = $state(chatState.currentChat?.systemPrompt || "");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");

  // 监听 currentChat 变化，更新本地状态
  $effect(() => {
    currentPrompt = chatState.currentChat?.systemPrompt || "";
    saveStatus = "saved";
  });

  // 自动保存：防抖处理
  $effect(() => {
    if (currentPrompt !== (chatState.currentChat?.systemPrompt || "")) {
      saveStatus = "saving";
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        try {
          console.log("currentPrompt: ", currentPrompt);
          await chatActions.updateSystemPrompt(currentPrompt);
          saveStatus = "saved";
        } catch (error) {
          console.error("Failed to update system prompt:", error);
          saveStatus = "error";
        }
      }, 500); // 500ms 防抖延迟
    }
  });
</script>

<div class="flex-1 p-0">
  <!-- 提示词编辑区 -->
  <TableGroup>
    <TextareaRow
      label="系统提示词"
      bind:value={currentPrompt}
      placeholder="输入系统提示词..."
      rows={6}
      showCharCount={true}
    >
      {#snippet rightContent()}
        {#if saveStatus !== "saved"}
          <div class="flex items-center gap-2 px-1">
            <span
              class="w-2 h-2 rounded-full {saveStatus === 'saving'
                ? 'bg-yellow-500'
                : 'bg-red-500'}"
            ></span>
            <span class="text-xs text-gray-500">
              {saveStatus === "saving" ? "保存中..." : "保存失败"}
            </span>
          </div>
        {/if}
      {/snippet}
    </TextareaRow>
  </TableGroup>
</div>
