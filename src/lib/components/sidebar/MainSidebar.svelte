<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import AgentProjectList from "$lib/components/agentsession/AgentProjectList.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import { t } from "$lib/i18n";
  import UserSidebar from "$lib/components/sidebar/UserSidebar.svelte";
  import {
    BookOpen,
    Bot,
    Settings,
    User,
    LogOut,
    Download,
    Clock,
  } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import { authState, login, logout, confirmLogout } from "$lib/states/auth.svelte";
  import { updateState } from "$lib/states/update.svelte";

  // 当前路由（用于常驻入口的高亮）
  let currentRoute = $derived(browser && $page.url ? $page.url.pathname : "");

  // 当前选中的 Agent 会话 ID（用于 AgentProjectList 高亮）
  let currentAgentSessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
  );

  function handleWordsClick() {
    goto(`/words`);
  }

  function handleAgentClick() {
    goto(`/agents`);
  }

  function handleJobsClick() {
    goto(`/jobs`);
  }

  // 从 authState 获取用户状态
  const currentUser = $derived({
    isLoggedIn: authState.isLoggedIn,
    username: authState.user?.username,
    email: authState.user?.email,
    avatar: authState.user?.avatar,
    isPro: authState.user?.isPro || false,
  });

  let showUserMenu = $state(false);
  let userMenuX = $state(0);
  let userMenuY = $state(0);
  let userMenuTrigger: HTMLDivElement | null = null;

  function openSettings(path?: string) {
    openSettingsWindow(path).catch((err) => {
      console.error("Failed to open settings window:", err);
    });
  }

  function handleUserClick(event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();

    if (showUserMenu) {
      showUserMenu = false;
      return;
    }

    if (event instanceof MouseEvent) {
      userMenuX = event.clientX;
      userMenuY = event.clientY;
    } else if (userMenuTrigger) {
      const rect = userMenuTrigger.getBoundingClientRect();
      userMenuX = rect.left;
      userMenuY = rect.top;
    }
    showUserMenu = true;
  }

  function handleUserMenuOutside(event: MouseEvent) {
    if (!showUserMenu) return;

    const target = event.target as HTMLElement;
    if (
      !target.closest(".user-context-menu") &&
      !target.closest(".user-menu-trigger")
    ) {
      showUserMenu = false;
    }
  }

  function handleMenuSettings() {
    showUserMenu = false;
    openSettings();
  }

  function handleMenuAccount() {
    showUserMenu = false;
    openSettings("/account");
  }

  async function handleMenuLogout() {
    showUserMenu = false;
    if (!(await confirmLogout())) {
      return;
    }
    await logout();
  }

  async function handleMenuLogin() {
    showUserMenu = false;
    await login();
  }
</script>

<div
  class="h-full flex flex-col p-0 pt-12 overflow-hidden"
>
  <!-- 顶部固定区域：常驻入口（任务 / Agents / 单词本） -->
  <div class="flex-shrink-0 flex flex-col px-2 space-y-0.5 mb-3">
    <MenuButton
      title={t("sidebar.jobs")}
      icon={Clock}
      iconSize={16}
      isActive={currentRoute === "/jobs"}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content/70 hover:text-base-content font-normal"
      onclick={() => handleJobsClick()}
    />
    <MenuButton
      title="Agents"
      icon={Bot}
      iconSize={16}
      isActive={currentRoute === "/agents"}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content/70 hover:text-base-content font-normal"
      onclick={() => handleAgentClick()}
    />
    <MenuButton
      title={t("sidebar.words")}
      icon={BookOpen}
      iconSize={16}
      isActive={currentRoute === "/words"}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content/70 hover:text-base-content font-normal"
      onclick={() => handleWordsClick()}
    />
  </div>

  <!-- 中间可滚动区域：Agent 会话列表（按项目分组） -->
  <div class="flex-1 min-h-0">
    <AgentProjectList activeId={currentAgentSessionId} />
  </div>

  <!-- 检测到更新：底部更新入口 -->
  {#if updateState.hasUpdate}
    <div class="flex-shrink-0 px-2 pt-1">
      <button
        type="button"
        class="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg bg-primary/10 text-primary hover:bg-primary/15 transition-colors text-[12px] font-medium"
        onclick={() => updateState.openDialog()}
      >
        <Download size={14} />
        <span>{t("sidebar.updateAvailable")}</span>
        {#if updateState.info?.version}
          <span class="ml-auto text-[11px] text-primary/70"
            >v{updateState.info.version}</span
          >
        {/if}
      </button>
    </div>
  {/if}

  <!-- 用户信息 -->
  <div
    class="flex-shrink-0 p-2 user-menu-trigger"
    bind:this={userMenuTrigger}
  >
    <UserSidebar user={currentUser} onUserClick={handleUserClick} />
  </div>

  {#if showUserMenu}
    <div
      class="user-context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
      style="left: {userMenuX}px; top: {userMenuY}px; transform: translateY(calc(-100% - 8px));"
      role="menu"
    >
      {#if currentUser.isLoggedIn}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuAccount}
        >
          <User size={14} />
          {t("common.account")}
        </button>
      {:else}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuLogin}
        >
          {t("common.login")}
        </button>
      {/if}

      <div class="border-t border-base-300 my-1 mx-2"></div>

      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
        onclick={handleMenuSettings}
      >
        <Settings size={14} />
        {t("common.settings")}
      </button>

      {#if currentUser.isLoggedIn}
        <div class="border-t border-base-300 my-1 mx-2"></div>
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuLogout}
        >
          <LogOut size={14} />
          {t("common.logout")}
        </button>
      {/if}
    </div>
  {/if}
</div>

<svelte:window onclick={handleUserMenuOutside} />
