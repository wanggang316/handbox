<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { messageStore } from "$lib/states";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import ChatList from "$lib/components/ui/ChatList.svelte";
  import AgentProjectList from "$lib/components/agentsession/AgentProjectList.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import { uiState } from "$lib/states/ui.svelte";
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

  // 获取当前选中的聊天 ID
  let currentChatId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
  );

  // 获取当前路由
  let currentRoute = $derived(
    browser && $page.url ? $page.url.pathname : ""
  );

  // 将真实聊天数据转换为 Menu 组件期望的格式
  let chats = $derived(
    chatState.chats
      .filter((chat) => chat.id) // 过滤掉没有 id 的聊天
      .map((chat) => ({
        id: chat.id!,
        title: chat.name,
      }))
  );

  function handleChatClick(chat: any) {
    console.log("Clicked chat:", chat);
    // 使用 SvelteKit 的客户端路由导航，避免页面重新加载
    goto(`/chat?id=${chat.id}`);
  }

  function handleWordsClick() {
    console.log("Clicked words menu");
    goto(`/words`);
  }

  function handleAgentClick() {
    console.log("Clicked agent menu");
    goto(`/agents`);
  }

  function handleJobsClick() {
    goto(`/jobs`);
  }

  // 新建会话（Chat 模式专属操作，入口为 ChatList 标题右侧的加号）
  function handleNewChat() {
    goto(`/chat`);
  }

  // 当前选中的 Agent 会话 ID（用于 AgentProjectList 高亮）
  let currentAgentSessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
  );

  // 切换到 Chat 模式：重复点击当前激活段为 no-op
  function selectChatMode() {
    if (uiState.appMode === "chat") return;
    uiState.setAppMode("chat");
    goto("/chat");
  }

  // 切换到 Agent 模式：重复点击为 no-op；有上次会话则恢复
  function selectAgentMode() {
    if (uiState.appMode === "agent") return;
    uiState.setAppMode("agent");
    const lastId = uiState.lastAgentSessionId;
    goto(lastId ? `/agent?id=${lastId}` : "/agent");
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

  // 处理聊天重命名
  async function handleChatRename(chat: any, newName: string) {
    try {
      await chatActions.renameChat(chat.id, newName);
      console.log("Chat renamed successfully:", chat.id, newName);
    } catch (error) {
      console.error("Failed to rename chat:", error);
      // 这里可以显示错误提示
    }
  }

  // 处理聊天删除
  async function handleChatDelete(chat: any) {
    try {
      await chatActions.deleteChat(chat.id);
      console.log("Chat deleted successfully:", chat.id);

      // 如果删除的是当前聊天，跳转到默认页面
      if (currentChatId === chat.id) {
        goto("/chat");
      }
    } catch (error) {
      console.error("Failed to delete chat:", error);
      // 这里可以显示错误提示
    }
  }

  // 处理生成标题（接收ChatList组件生成的标题）
  async function handleGenerateTitle(chat: any, newTitle: string) {
    try {
      await chatActions.renameChat(chat.id, newTitle);
      console.log("Chat title updated successfully:", chat.id, newTitle);
    } catch (error) {
      console.error("Failed to update generated title:", error);
      // 这里可以显示错误提示
    }
  }

</script>

<div
  class="h-full flex flex-col p-0 pt-12 overflow-hidden"
>
  <!-- 顶部固定区域 -->
  <div class="flex-shrink-0 space-y-3 mb-3">
    <!-- 全局入口：任务 -->
    <div class="flex flex-col px-2 space-y-0.5">
      <MenuButton
        title={t("sidebar.jobs")}
        icon={Clock}
        iconSize={16}
        isActive={currentRoute === "/jobs"}
        buttonClass="px-2 py-1 text-[12px] leading-[18px] text-base-content/70 hover:text-base-content font-normal"
        onclick={() => handleJobsClick()}
      />
    </div>

    <!-- Chat | Agent 模式分段控件 -->
    <div class="px-2">
      <div class="flex p-0.5 bg-base-300 rounded-md">
        <button
          class="flex-1 py-1 text-[12px] leading-[18px] rounded-[5px] font-normal transition-colors {uiState.appMode ===
          'chat'
            ? 'bg-base-100 text-base-content shadow-sm'
            : 'text-base-content/60 hover:text-base-content'}"
          aria-pressed={uiState.appMode === "chat"}
          onclick={selectChatMode}
        >
          Chat
        </button>
        <button
          class="flex-1 py-1 text-[12px] leading-[18px] rounded-[5px] font-normal transition-colors {uiState.appMode ===
          'agent'
            ? 'bg-base-100 text-base-content shadow-sm'
            : 'text-base-content/60 hover:text-base-content'}"
          aria-pressed={uiState.appMode === "agent"}
          onclick={selectAgentMode}
        >
          Agent
        </button>
      </div>
    </div>
  </div>

  <!-- 中间可滚动区域 -->
  <div class="flex-1 min-h-0">
    {#if uiState.appMode === "agent"}
      <AgentProjectList activeId={currentAgentSessionId} />
    {:else}
      <div class="flex flex-col h-full">
        <!-- Chat 模式专属入口：Agents、单词本 -->
        <div class="flex-shrink-0 flex flex-col px-2 space-y-0.5 mb-3">
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

        <!-- 聊天列表（新建会话入口在列表标题右侧） -->
        <div class="flex-1 min-h-0">
          <ChatList
            {chats}
            activeId={currentChatId}
            streamingChatId={messageStore.streamingChatId}
            onChatClick={handleChatClick}
            onNewChat={handleNewChat}
            onRename={handleChatRename}
            onDelete={handleChatDelete}
            onGenerateTitle={handleGenerateTitle}
          />
        </div>
      </div>
    {/if}
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
