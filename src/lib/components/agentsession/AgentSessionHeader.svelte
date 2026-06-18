<script lang="ts">
  /**
   * Agent 会话头部：显示当前会话的名称 + 模型 + (可选) 工作目录 / 思考强度。
   * 由 `agentSessionState.currentSession` 驱动，故重新打开会话时配置即可见。
   *
   * 右侧设置按钮打开 System Prompt popover —— 配置弹窗删除后，
   * 这是全部 session（含未分组旧 session）唯一的 prompt 编辑入口。
   */
  import {
    Bot,
    FolderOpen,
    Brain,
    Settings2,
    ExternalLink,
    ChevronDown,
    Code2,
    SquareTerminal,
    Check,
  } from "@lucide/svelte";
  import IconButton from "../ui/IconButton.svelte";
  import { t } from "$lib/i18n";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import {
    listOpenInTargets,
    openInTarget,
    type OpenInTarget,
  } from "$lib/api/openIn";
  import { settingsState } from "$lib/states/settings.svelte";

  const session = $derived(agentSessionState.currentSession);

  // ============================================
  // "Open in ..." 分体按钮 + 下拉
  // ============================================
  // 把当前会话工作目录在外部 editor / terminal / Finder 中打开。探测、取图标与
  // 启动都在后端（commands/open_in.rs）；目标清单按已安装 app 维度缓存（会话内
  // 不变），有工作目录的会话挂载时即预取，使分体按钮能立刻显示默认应用图标。
  let openInMenuOpen = $state(false);
  let openInTargets = $state<OpenInTarget[] | null>(null);
  let openInLoading = $state(false);
  let openInError = $state<string | null>(null);

  // 已存的默认应用 id（持久化在 agent 设置里，跨会话 / 重启生效）。
  const defaultEditorId = $derived(
    settingsState.settings?.agent?.defaultEditorId ?? null,
  );

  // 解析默认 target：已存默认仍可用则取之；否则回退到首个 editor/terminal；
  // 再不行取首个（通常是 Finder）。供分体按钮的主操作使用。
  const resolvedDefault = $derived.by((): OpenInTarget | null => {
    const targets = openInTargets;
    if (!targets || targets.length === 0) return null;
    return (
      targets.find((t) => t.id === defaultEditorId) ??
      targets.find((t) => t.kind !== "system") ??
      targets[0] ??
      null
    );
  });

  function iconForKind(kind: OpenInTarget["kind"]) {
    if (kind === "terminal") return SquareTerminal;
    if (kind === "system") return FolderOpen;
    return Code2;
  }

  async function loadOpenInTargets() {
    if (openInTargets !== null || openInLoading) return;
    openInLoading = true;
    openInError = null;
    try {
      openInTargets = await listOpenInTargets();
    } catch (error) {
      openInError = error instanceof Error ? error.message : String(error);
    } finally {
      openInLoading = false;
    }
  }

  // 有工作目录时即预取目标清单与设置，使分体按钮的默认应用图标无需等点击。
  // 两个加载都幂等（targets 自带去重守卫，loadSettings 命中缓存即返回）。
  $effect(() => {
    if (session?.workingDir) {
      void loadOpenInTargets();
      void settingsState.loadSettings();
    }
  });

  function toggleOpenInMenu(event: MouseEvent) {
    // 阻止冒泡到 window 的 click-outside（否则刚打开即被关闭）。
    event.stopPropagation();
    if (openInMenuOpen) {
      closeOpenInMenu();
      return;
    }
    // 互斥：打开本菜单时关掉 System Prompt popover。
    closePromptPopover();
    openInMenuOpen = true;
    openInError = null;
    void loadOpenInTargets();
  }

  function closeOpenInMenu() {
    openInMenuOpen = false;
    openInError = null;
  }

  // 仅打开、不改默认（分体按钮主操作 / 一次性）。成功收起菜单，失败留错误条。
  async function openTarget(target: OpenInTarget) {
    const dir = session?.workingDir;
    if (!dir) return;
    try {
      await openInTarget(dir, target.id);
      closeOpenInMenu();
    } catch (error) {
      // 启动失败不静默：菜单留在原地、错误条可见。
      openInError = error instanceof Error ? error.message : String(error);
    }
  }

  // 从下拉选择：打开；若是 editor/terminal 则记为默认（「默认编辑器」语义不含
  // Finder）。持久化失败不阻断打开，仅记日志。
  async function pickTarget(target: OpenInTarget) {
    if (target.kind !== "system" && target.id !== defaultEditorId) {
      settingsState
        .updateSettings({
          section: "agent",
          data: { defaultEditorId: target.id },
        })
        .catch((error) => console.error("设置默认编辑器失败:", error));
    }
    await openTarget(target);
  }

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
    // 互斥：打开 System Prompt popover 时关掉 Open-in 菜单。
    closeOpenInMenu();
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
    if (event.key !== "Escape") return;
    if (promptPopoverOpen) closePromptPopover();
    if (openInMenuOpen) closeOpenInMenu();
  }

  // 点击 popover / 菜单外任意处关闭（closest 检查同 AgentProjectList 菜单）。
  function handleWindowClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (promptPopoverOpen && !target.closest(".sysprompt-popover")) {
      closePromptPopover();
    }
    if (openInMenuOpen && !target.closest(".openin-popover")) {
      closeOpenInMenu();
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

  // 会话切换时一并关闭 Open-in 菜单（targets 与系统相关、不随会话变，但悬置的
  // 菜单留到新会话会造成心智错位——与 System Prompt popover 的关闭语义一致）。
  // 用普通 let 记上一次 id：非响应式，避免在 effect 内自我触发。
  let lastOpenInSessionId: string | null = null;
  $effect(() => {
    const id = session?.id ?? null;
    if (id !== lastOpenInSessionId) {
      lastOpenInSessionId = id;
      if (openInMenuOpen) closeOpenInMenu();
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
    <div class="relative z-[10000] ml-auto shrink-0 flex items-center gap-1">
      <!-- Open in …：把工作目录在外部 editor / terminal / Finder 中打开。
           分体按钮——左：在默认应用打开（显示其真实图标）；右：展开应用列表。
           仅当会话有工作目录时出现（无目录则无从打开）。 -->
      {#if session.workingDir}
        <div
          class="flex items-center rounded-md overflow-hidden border border-base-300/60"
        >
          <button
            type="button"
            class="h-7 pl-1.5 pr-1 flex items-center bg-transparent text-base-content hover:bg-base-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title={resolvedDefault
              ? `在 ${resolvedDefault.name} 中打开`
              : "Open in…"}
            aria-label="在默认应用中打开"
            disabled={!resolvedDefault}
            onclick={() => resolvedDefault && openTarget(resolvedDefault)}
          >
            {#if resolvedDefault?.icon}
              <img
                src={resolvedDefault.icon}
                alt=""
                class="w-4 h-4 rounded-[3px]"
              />
            {:else}
              <ExternalLink size={15} />
            {/if}
          </button>
          <button
            type="button"
            class="h-7 px-0.5 flex items-center bg-transparent text-base-content/70 hover:bg-base-300 transition-colors border-l border-base-300/60"
            title="选择应用"
            aria-label="选择要打开的应用"
            onclick={toggleOpenInMenu}
          >
            <ChevronDown size={13} />
          </button>
        </div>

        {#if openInMenuOpen}
          <div
            class="openin-popover absolute right-0 top-full mt-2 z-[10020] min-w-48 max-w-[80vw] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl p-1 flex flex-col max-h-96 overflow-y-auto"
          >
            {#if openInError}
              <div
                class="m-1 text-xs text-error bg-error/10 border border-error/20 rounded-md px-2 py-1.5 break-words"
              >
                {openInError}
              </div>
            {/if}

            {#if openInLoading}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                检测可用应用…
              </div>
            {:else if openInTargets && openInTargets.length > 0}
              {#each openInTargets as target (target.id)}
                {@const FallbackIcon = iconForKind(target.kind)}
                {@const isDefault =
                  target.kind !== "system" && target.id === defaultEditorId}
                <button
                  type="button"
                  class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
                  onclick={() => pickTarget(target)}
                >
                  {#if target.icon}
                    <img
                      src={target.icon}
                      alt=""
                      class="w-4 h-4 rounded-[3px] shrink-0"
                    />
                  {:else}
                    <FallbackIcon size={14} class="shrink-0" />
                  {/if}
                  <span class="flex-1 truncate">{target.name}</span>
                  {#if isDefault}
                    <Check size={13} class="shrink-0 opacity-70" />
                  {/if}
                </button>
              {/each}
            {:else if !openInError}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                未检测到可用应用
              </div>
            {/if}
          </div>
        {/if}
      {/if}

      <IconButton
        icon={Settings2}
        iconSize={16}
        ariaLabel={t("agent.systemPrompt.editAria")}
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
            placeholder={t("agent.systemPrompt.placeholder")}
            rows="6"
            class="w-full min-h-28 max-h-64 px-3 py-2 border border-base-300 rounded-md resize-y
                   focus:border-transparent
                   font-mono text-sm text-base-content bg-base-200"
          ></textarea>

          {#if promptError}
            <div
              class="text-xs text-error bg-error/10 border border-error/20 rounded-md px-2 py-1.5 break-words"
            >
              {t("agent.systemPrompt.saveFailed", { error: promptError })}
            </div>
          {/if}

          <div class="flex items-center justify-end gap-2">
            <button
              type="button"
              class="rounded-full border border-base-300 px-3 py-1 text-xs font-medium text-base-content hover:border-base-300/70 hover:bg-base-200 transition-colors"
              onclick={closePromptPopover}
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              class="rounded-full border border-base-300 px-3 py-1 text-xs font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors disabled:opacity-60 disabled:cursor-not-allowed"
              disabled={isSavingPrompt}
              onclick={savePrompt}
            >
              {isSavingPrompt ? t("common.saving") : t("common.save")}
            </button>
          </div>
        </div>
      {/if}
    </div>
  </header>
{/if}

<!-- 全局监听：Esc / 点击 popover 外关闭并丢弃草稿（按钮 click 已 stopPropagation） -->
<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />
