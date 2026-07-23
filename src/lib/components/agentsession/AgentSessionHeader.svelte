<script lang="ts">
  /**
   * Agent 会话头部：显示当前会话的 项目（可选）/ 标题。
   * 由 `agentSessionState.currentSession` 驱动，故重新打开会话时即可见。
   */
  import {
    FolderOpen,
    ExternalLink,
    ChevronDown,
    Code2,
    SquareTerminal,
    Check,
  } from "@lucide/svelte";
  import { agentSessionState } from "$lib/states/agentSession.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import {
    listOpenInTargets,
    openInTarget,
    type OpenInTarget,
  } from "$lib/api/openIn";
  import { settingsState } from "$lib/states/settings.svelte";
  import { uiState } from "$lib/states/ui.svelte";

  const session = $derived(agentSessionState.currentSession);

  // 所属项目名（可选）：projects 由侧栏 AgentProjectList 加载，此处响应式读取；
  // 未挂项目或列表未就绪时为 null，仅显示标题。
  const projectName = $derived.by(() => {
    const projectId = session?.projectId;
    if (!projectId) return null;
    return (
      agentProjectState.projects.find((p) => p.id === projectId)?.name ?? null
    );
  });

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

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (openInMenuOpen) closeOpenInMenu();
  }

  // 点击菜单外任意处关闭（closest 检查同 AgentProjectList 菜单）。
  function handleWindowClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (openInMenuOpen && !target.closest(".openin-popover")) {
      closeOpenInMenu();
    }
  }

  // 会话切换时关闭 Open-in 菜单（targets 与系统相关、不随会话变，但悬置的
  // 菜单留到新会话会造成心智错位）。
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
  <!-- 侧栏收起时主内容顶到窗口左缘：给头部让出红绿灯 + 侧栏开关的位置
       （toggle 位于 left:100px，按钮宽约 30px），padding 过渡与侧栏动画同步。 -->
  <header
    class="flex items-center gap-3 px-4 py-2.5 border-b border-base-300 shrink-0 transition-[padding-left] duration-[var(--dur-base)] {uiState.sidebarOpen
      ? ''
      : 'pl-[136px]'}"
  >
    <div class="flex items-center gap-2 min-w-0 text-sm h-7">
      {#if projectName}
        <span class="text-base-content/50 truncate shrink-0 max-w-[40%]">
          {projectName}
        </span>
        <span class="text-base-content/30 shrink-0">/</span>
      {/if}
      <span class="font-medium text-base-content truncate">
        {session.name}
      </span>
    </div>

    <!-- z-[10000]：header 顶部条带被 TitleBar 的 .drag-region（fixed,
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
            class="openin-popover absolute right-0 top-full mt-2 z-[var(--z-popover)] min-w-48 max-w-[80vw] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl p-1 flex flex-col max-h-96 overflow-y-auto"
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
                  class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
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
    </div>
  </header>
{/if}

<!-- 全局监听：Esc / 点击菜单外关闭（按钮 click 已 stopPropagation） -->
<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />
