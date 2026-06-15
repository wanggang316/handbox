<script lang="ts">
  /**
   * Agent 会话头部：显示当前会话的名称 + 模型 + (可选) 工作目录 / 思考强度。
   * 由 `agentSessionState.currentSession` 驱动，故重新打开会话时配置即可见。
   *
   * 右侧设置按钮打开 System Prompt popover —— 配置弹窗删除后，
   * 这是全部 session（含未分组旧 session）唯一的 prompt 编辑入口。
   */
  import { Bot, FolderOpen, Brain, Settings2 } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";

  const session = $derived(agentSessionState.currentSession);

  // ============================================
  // System Prompt popover
  // ============================================
  // 草稿绑定「打开时刻」的 sessionId：保存显式写回 capture 的 id，
  // 异步保存途中切换会话也不会写错目标（写回原 session，next-run 语义不变）。
  let promptSessionId = $state<string | null>(null);
  let promptDraft = $state("");
  let promptError = $state<string | null>(null);
  let isSavingPrompt = $state(false);

  const promptPopoverOpen = $derived(promptSessionId !== null);

  function togglePromptPopover(event: MouseEvent) {
    // 阻止冒泡到 window 的 click-outside（否则刚打开即被关闭）。
    event.stopPropagation();
    if (promptPopoverOpen) {
      closePromptPopover();
      return;
    }
    if (!session) return;
    promptSessionId = session.id;
    promptDraft = session.systemPrompt ?? "";
    promptError = null;

    // 等 popover 挂载后聚焦 textarea（与 AgentProjectList 重命名的定位方式一致）。
    setTimeout(() => {
      const textarea = document.querySelector(
        ".sysprompt-popover textarea",
      ) as HTMLTextAreaElement | null;
      textarea?.focus();
    }, 0);
  }

  /** 三途径关闭（Esc / 点击外部 / 取消按钮）一律丢弃草稿、不写库。 */
  function closePromptPopover() {
    promptSessionId = null;
    promptDraft = "";
    promptError = null;
  }

  async function savePrompt() {
    const targetId = promptSessionId;
    if (!targetId || isSavingPrompt) return;
    // 纯空白视同清空：传空串（后端存 Some("")，run 行为等同未设置）。
    const value = promptDraft.trim() === "" ? "" : promptDraft;
    isSavingPrompt = true;
    promptError = null;
    try {
      await agentSessionActions.updateField(targetId, "systemPrompt", value);
      closePromptPopover();
    } catch (error) {
      // 保存失败不静默：popover 留在原地、草稿保留、错误条可见。
      promptError = error instanceof Error ? error.message : String(error);
    } finally {
      isSavingPrompt = false;
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (promptPopoverOpen && event.key === "Escape") {
      closePromptPopover();
    }
  }

  // 点击 popover 外任意处关闭并丢弃草稿（closest 检查同 AgentProjectList 菜单）。
  function handleWindowClick(event: MouseEvent) {
    if (!promptPopoverOpen) return;
    const target = event.target as HTMLElement;
    if (!target.closest(".sysprompt-popover")) {
      closePromptPopover();
    }
  }

  // 会话切换或清空（如被删除）时关闭 popover 并丢弃草稿（VAL-CREATE-024
  // 取「关闭」分支）：header 已显示新会话，留着绑定旧会话草稿的 popover
  // 会造成心智错位。打开瞬间 promptSessionId 即为当前会话 id，不会误关。
  $effect(() => {
    if (promptPopoverOpen && session?.id !== promptSessionId) {
      closePromptPopover();
    }
  });
</script>

{#if session}
  <header
    class="flex items-center gap-3 px-4 py-2.5 border-b border-base-300 shrink-0"
  >
    <Bot size={18} class="opacity-60 shrink-0" />
    <div class="flex flex-col min-w-0">
      <span class="text-sm font-medium text-base-content truncate">
        {session.name}
      </span>
      <div
        class="flex items-center gap-3 text-xs text-base-content/50 mt-0.5"
      >
        {#if session.modelId}
          <span class="truncate">{session.modelId}</span>
        {/if}
        {#if session.thinkingLevel && session.thinkingLevel !== "off"}
          <span class="flex items-center gap-1 shrink-0">
            <Brain size={12} />
            {session.thinkingLevel}
          </span>
        {/if}
        {#if session.workingDir}
          <span class="flex items-center gap-1 min-w-0">
            <FolderOpen size={12} class="shrink-0" />
            <span class="truncate">{session.workingDir}</span>
          </span>
        {/if}
      </div>
    </div>

    <!-- System Prompt 设置入口 + popover（相对容器锚定，右对齐展开）。
         z-[10000]：header 顶部条带被 TitleBar 的 .drag-region（fixed,
         height:50px, z-index:9999）覆盖，按钮 mousedown 会触发窗口拖拽而非
         点击；提升到拖拽层之上（镜像 TitleBar 自身按钮的 z-index:10000
         模式），popover 卡片随容器一并抬升，内部交互不被吞。 -->
    <div class="relative z-[10000] ml-auto shrink-0">
      <IconButton
        icon={Settings2}
        iconSize={16}
        ariaLabel="编辑 System Prompt"
        title="System Prompt"
        onclick={togglePromptPopover}
      />

      {#if promptPopoverOpen}
        <div
          class="sysprompt-popover absolute right-0 top-full mt-2 z-[10020] w-96 max-w-[80vw] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl p-3 flex flex-col gap-2"
        >
          <h3 class="text-sm font-medium text-base-content">System Prompt</h3>

          <textarea
            bind:value={promptDraft}
            placeholder="输入系统提示词..."
            rows="6"
            class="w-full min-h-28 max-h-64 px-3 py-2 border border-base-300 rounded-md resize-y
                   focus:border-transparent
                   font-mono text-sm text-base-content bg-base-200"
          ></textarea>

          {#if promptError}
            <div
              class="text-xs text-error bg-error/10 border border-error/20 rounded-md px-2 py-1.5 break-words"
            >
              保存失败：{promptError}
            </div>
          {/if}

          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              class="rounded-full border border-base-300 px-3 py-1 text-xs font-medium text-base-content hover:border-base-300/70 hover:bg-base-200 transition-colors"
              onclick={closePromptPopover}
            >
              取消
            </button>
            <button
              type="button"
              class="rounded-full border border-base-300 px-3 py-1 text-xs font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors disabled:opacity-60 disabled:cursor-not-allowed"
              disabled={isSavingPrompt}
              onclick={savePrompt}
            >
              {isSavingPrompt ? "保存中…" : "保存"}
            </button>
          </div>
        </div>
      {/if}
    </div>
  </header>
{/if}

<!-- 全局监听：Esc / 点击 popover 外关闭并丢弃草稿（按钮 click 已 stopPropagation） -->
<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />
