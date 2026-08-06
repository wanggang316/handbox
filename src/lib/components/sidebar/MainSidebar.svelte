<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import AgentProjectList from "$lib/components/agentsession/AgentProjectList.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import { t } from "$lib/i18n";
  import UserSidebar from "$lib/components/sidebar/UserSidebar.svelte";
  import {
    Bot,
    Settings,
    SquarePen,
    User,
    LogIn,
    LogOut,
    Download,
    Clock,
  } from "@lucide/svelte";
  import { authState, login, logout, confirmLogout } from "$lib/states/auth.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { toastActions } from "$lib/states/toast.svelte";
  import { normalizeError } from "$lib/utils/error";

  let currentRoute = $derived(browser && $page.url ? $page.url.pathname : "");

  let currentAgentSessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
  );

  function handleAgentClick() {
    goto(`/agents`);
  }

  function handleJobsClick() {
    goto(`/jobs`);
  }

  // Seeded general-chat AgentDefinition (working_dir_mode "none": pure dialog).
  const BUILTIN_CHAT_AGENT_ID = "builtin-chat";

  // New Chat is idempotent, Claude-style: an existing empty general-chat
  // session (no project, no persisted turns, no active run) is reopened
  // instead of stacking up blank sessions on repeated clicks.
  async function handleNewChatClick() {
    const reusable = agentSessionState.sessions.find(
      (session) =>
        session.agentDefinitionId === BUILTIN_CHAT_AGENT_ID &&
        !session.projectId &&
        session.messageCount === 0 &&
        !agentRunStore.runStateFor(session.id).isRunning &&
        agentRunStore.runStateFor(session.id).messages.length === 0,
    );
    if (reusable) {
      goto(`/agent?id=${reusable.id}`);
      return;
    }
    try {
      const session = await agentSessionActions.createSessionFromDefinition(
        BUILTIN_CHAT_AGENT_ID,
      );
      goto(`/agent?id=${session.id}`);
    } catch (error) {
      console.error("Failed to create chat session:", error);
      const normalized = normalizeError(error, t("agent.list.createSessionFailed"));
      toastActions.error(normalized.hint ?? normalized.message);
    }
  }

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
  let userMenuWidth = $state(0);
  let userMenuTrigger: HTMLDivElement | null = null;

  // Settings renders inside the main window, so plain routing is enough.
  function openSettings(path?: string) {
    goto(path ? `/settings${path}` : "/settings");
  }

  function handleUserClick(event: MouseEvent | KeyboardEvent) {
    event.preventDefault();
    event.stopPropagation();

    if (showUserMenu) {
      showUserMenu = false;
      return;
    }

    // Anchor to the sidebar user row: left-aligned, same width as the content area, opening upward.
    if (userMenuTrigger) {
      const rect = userMenuTrigger.getBoundingClientRect();
      userMenuX = rect.left + 8;
      userMenuY = rect.top;
      userMenuWidth = rect.width - 16;
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
  <div class="flex-shrink-0 flex flex-col px-2 space-y-0.5 mb-3">
    <MenuButton
      title={t("sidebar.newChat")}
      icon={SquarePen}
      iconSize={16}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content font-normal"
      onclick={() => handleNewChatClick()}
    />
    <MenuButton
      title={t("sidebar.jobs")}
      icon={Clock}
      iconSize={16}
      isActive={currentRoute === "/jobs"}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content font-normal"
      onclick={() => handleJobsClick()}
    />
    <MenuButton
      title="Agents"
      icon={Bot}
      iconSize={16}
      isActive={currentRoute === "/agents"}
      buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content font-normal"
      onclick={() => handleAgentClick()}
    />
  </div>

  <div class="flex-1 min-h-0">
    <AgentProjectList activeId={currentAgentSessionId} />
  </div>

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

  <div
    class="flex-shrink-0 p-2 user-menu-trigger"
    bind:this={userMenuTrigger}
  >
    <UserSidebar user={currentUser} onUserClick={handleUserClick} />
  </div>

  {#if showUserMenu}
    <div
      class="user-context-menu fixed z-[var(--z-dropdown)] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1"
      style="left: {userMenuX}px; top: {userMenuY}px; width: {userMenuWidth}px; transform: translateY(calc(-100% - 4px));"
      role="menu"
    >
      {#if currentUser.isLoggedIn}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuAccount}
        >
          <User size={14} />
          {t("common.account")}
        </button>
      {:else}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuLogin}
        >
          <LogIn size={14} />
          {t("common.login")}
        </button>
      {/if}

      <div class="border-t border-base-300 my-1 mx-2"></div>

      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
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
