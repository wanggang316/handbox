<script lang="ts">
  import { chatState, chatActions } from "$lib/states/chat.svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import ChatList from "$lib/components/ui/ChatList.svelte";
  import MenuButton from "$lib/components/ui/MenuButton.svelte";
  import UserSidebar from "$lib/components/sidebar/UserSidebar.svelte";
  import { BookOpen, Box, Search, Settings, User, LogOut, Star } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import { authState, login, logout, confirmLogout } from "$lib/states/auth.svelte";
  import SearchModal from "$lib/components/search/SearchModal.svelte";

  // 获取当前选中的聊天 ID
  let currentChatId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : ""
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

  function handleArtifactClick() {
    console.log("Clicked artifact menu");
    goto(`/artifacts`);
  }

  function handleFavoriteClick() {
    console.log("Clicked favorite menu");
    goto(`/favorites`);
  }

  function handleWordsClick() {
    console.log("Clicked words menu");
    goto(`/words`);
  }

  let showSearchModal = $state(false);

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
  class="h-full flex flex-col bg-base-200 p-0 pt-12 rounded-2xl overflow-hidden"
>
  <!-- 顶部固定区域 -->
  <div class="flex-shrink-0 space-y-6 mb-6">
    <!-- 搜索框 -->
    <div class="px-2">
      <div class="relative">
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
          size={16}
        />
        <input
          type="text"
          placeholder="搜索..."
          class="w-full h-8 pl-10 pr-4 bg-base-300 rounded-lg text-base-content placeholder:text-base-content/50 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
          onfocus={() => (showSearchModal = true)}
          readonly
        />
      </div>
    </div>

    <div class="flex px-2">
      <MenuButton
        title="收藏"
        icon={Star}
        iconSize={20}
        onClick={() => handleFavoriteClick()}
      />
      <MenuButton
        title="Artifacts"
        icon={Box}
        iconSize={20}
        onClick={() => handleArtifactClick()}
      />
      <MenuButton
        title="单词本"
        icon={BookOpen}
        iconSize={20}
        onClick={() => handleWordsClick()}
      />
    </div>
  </div>

  <!-- 中间可滚动区域 -->
  <div class="flex-1 min-h-0">
    <ChatList
      {chats}
      activeId={currentChatId}
      onChatClick={handleChatClick}
      onRename={handleChatRename}
      onDelete={handleChatDelete}
      onGenerateTitle={handleGenerateTitle}
    />
  </div>

  <!-- 用户信息 -->
  <div
    class="flex-shrink-0 p-2 user-menu-trigger"
    bind:this={userMenuTrigger}
  >
    <UserSidebar user={currentUser} onUserClick={handleUserClick} />
  </div>

  {#if showUserMenu}
    <div
      class="user-context-menu fixed z-[10020] bg-base-100 border border-base-300 rounded-xl shadow-xl px-1 py-1 min-w-36"
      style="left: {userMenuX}px; top: {userMenuY}px; transform: translateY(calc(-100% - 8px));"
      role="menu"
    >
      {#if currentUser.isLoggedIn}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuAccount}
        >
          <User size={14} />
          账号
        </button>
      {:else}
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuLogin}
        >
          登录
        </button>
      {/if}

      <div class="border-t border-base-300 my-1 mx-2"></div>

      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
        onclick={handleMenuSettings}
      >
        <Settings size={14} />
        设置
      </button>

      {#if currentUser.isLoggedIn}
        <div class="border-t border-base-300 my-1 mx-2"></div>
        <button
          class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
          onclick={handleMenuLogout}
        >
          <LogOut size={14} />
          退出
        </button>
      {/if}
    </div>
  {/if}

  <SearchModal bind:open={showSearchModal} />
</div>

<svelte:window onclick={handleUserMenuOutside} />
